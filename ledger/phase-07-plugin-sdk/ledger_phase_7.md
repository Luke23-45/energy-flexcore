# Phase 7 — Plugin SDK and Modular Composition: Ledger

| Field | Value |
|---|---|
| Phase | 7 — Stage 7 Plugin SDK and modular composition |
| Report trace | §23 Stage 7; §5; §9; §22.2; §2.1 (v0.5.0) |
| Release | v0.5.0 — Plugin SDK |
| Control status | **Simulation / shadow. No plugin can bypass dispatch gates.** No plugin
  can select or raise control-authority mode (§5.7, §21.4). |
| Status | Ledger only — no code written, no decisions closed |
| Research cut-off | 7 November 2026 (report §27) |

## How to read this ledger

- Every subtask has an ID (`P7.01` …), a report trace (`§N` / `§N.M`),
  inputs, detailed steps, a concrete deliverable, testable acceptance
  criteria, an owner role, dependencies, a verification method, and risks.
  **No subtask is done until its acceptance criteria are evidenced.**
- Checkboxes `- [ ]` mark work not yet done. Tick a box only when the named
  deliverable exists and the verification method has been run and recorded.
- Canonical units and signs (§10.2): MW, MVAr, MVA, MWh; `P > 0` = export
  at the point of connection; reactive-power sign declared per grid model
  and device profile, never assumed; SOC is a fraction in [0,1] at the API
  boundary. Telemetry quality is exactly one of
  `GOOD` / `SUSPECT` / `BAD` / `MISSING` / `ESTIMATED` / `STALE` (§10.4).
- Command lifecycle (§14.1): `PROPOSED → VALIDATED → FEASIBLE → APPROVED →
  ISSUED → ACKNOWLEDGED → TRACKING → COMPLETED`, plus `REJECTED`, `FAILED`,
  `EXPIRED`, `CANCELLED`, `SUPERSEDED`, `FALLBACK`. Validation, feasibility,
  and authorization are distinct gates.
- **Hard boundary (ADR-006, §5.4): no arbitrary Rust dynamic-library
  loading.** Third-party extensions use WASM/WIT components or out-of-process
  gRPC workers. A `.so`/`.dll` plugin loader is not built in this phase, in
  any form, including via a C ABI wrapper as the default mechanism.
- **Protected kernel (§5.1, §5.7, §9.1):** authorization, tenant isolation,
  audit integrity, command lifecycle, and final dispatch validation are
  platform invariants. No plugin may replace them, implement
  `CommandAuthorizer`, or gain direct access to `DispatchGateway`.
- This phase does **not** certify anything and does not grant any plugin
  field dispatch authority. ML/RL outputs are proposals only (ADR-010).
- Open gates: D4 (license audit — the WASM/gRPC stack adds new dependencies)
  and the Phase 6 handover (P6.29 seams). D5/D6 do not gate this phase's
  SDK work, but any plugin that touches a site adapter inherits the Phase 6
  command-path gate (P6.14).

## Prior-phase outputs consumed (per §23 Stages 1–6)

- Phases 1–5: workspace, API, Postgres, NATS, telemetry, flexibility,
  deterministic scheduler, grid feasibility, command lifecycle, console,
  simulator (§23 Stages 1–5).
- Phase 6: Rust edge agent, outbound mTLS, OpenEMS integration, one
  protocol adapter with mapping tables and conformance suite, command path
  disabled by default with a written test-env gate, `site-edge` profile
  input, and the P6.29 handover listing the capability seams that exist
  (and those that do not). If P6.29 was not delivered, P7.04 must fail and
  this phase must not define WIT interfaces for adapters it has not seen.

---

## P7.01 — Phase readiness check: Phases 1–6 outputs and the P6.29 handover

- [ ] **P7.01 — Phase readiness check: Phases 1–6 outputs and the P6.29 handover**
  - **Objective:** Verify, by running the checks, that the capability seams
    this phase will publish actually exist in the code, and that the Phase 6
    handover is complete enough to build on.
  - **Report trace:** §23 Stages 1–6; §5.2; §22.2; AGENTS.md §6–§7.
  - **Inputs / prerequisites:** Phase 1–6 ledgers and exit-gate evidence;
    P6.29 handover note; repo at the Phase 6 state.
  - **Steps:**
    1. Run `python tools/verify_docs.py` and `python tools/validate_agent.py`; record actual output.
    2. Run the workspace test suite and record pass/fail.
    3. For each seam listed in P6.29, confirm the Rust port trait exists, is
       implemented by at least one provider, and is consumed only through
       the contract (grep for direct concrete imports in application/domain
       code — any hit is a gap).
    4. List every gap; block WIT/SDK design (P7.05 onward) on any gap in
       the optimizer, grid-model, telemetry, or adapter seams.
  - **Deliverable / output:** Readiness-check record with command outputs,
    seam-by-seam existence table, and gap list.
  - **Acceptance criteria / exit check:** Both doc tools pass; tests pass;
    every P6.29 seam verified in code or recorded as a gap with an owner;
    no application/domain code imports a concrete provider directly.
  - **Owner role:** Architect.
  - **Dependencies:** P6.29 (Phase 6).
  - **Verification method:** Re-run the quoted commands; re-grep the
    seam imports.
  - **Risks / failure modes:** Publishing a WIT interface for a contract
    that does not exist in Rust yet — the SDK then specifies fiction;
    "verified" claimed without running anything.

## P7.02 — Plugin SDK scope freeze: contracts, categories, and the non-replaceable kernel

- [ ] **P7.02 — Plugin SDK scope freeze: contracts, categories, and the non-replaceable kernel**
  - **Objective:** Freeze, in writing, which capability contracts the SDK
    v0.5.0 publishes, which plugin categories it supports, and what is
    explicitly not replaceable — before any interface is written.
  - **Report trace:** §5.1–§5.3; §9.1; §22.1 (ADR-005, ADR-006, ADR-010).
  - **Inputs / prerequisites:** P7.01 seam table; §5.3 category table;
    §9.1 interface list.
  - **Steps:**
    1. List the v0.5.0 contracts: `AssetAdapter`, `TelemetrySource`,
       `GridModelProvider`, `Optimizer`, `ForecastProvider`,
       `MarketAdapter` (§9.1), plus any Phase 6 adapter port — each with
       its owner and version.
    2. List the supported plugin categories for v0.5.0 from §5.3 and mark
       each as in-scope, deferred, or out-of-scope (e.g. UI extensions and
       research algorithms have different readiness; record the call).
    3. Write the non-replaceable list verbatim from §5.1/§5.7/§9.1:
       `CommandAuthorizer`, `DispatchGateway`, tenant isolation, audit
       integrity, command lifecycle, final dispatch validation.
    4. Record the decision as an ADR if it changes any §22.1 ADR's scope;
       otherwise record it as the SDK scope note.
  - **Deliverable / output:** SDK scope note (contracts, categories,
    non-replaceable list) + ADR if required.
  - **Acceptance criteria / exit check:** Every published contract names
    its owner and version; every §5.3 category has an in/deferred/out
    verdict; the non-replaceable list is present verbatim and no contract
    in the list contradicts it.
  - **Owner role:** Architect.
  - **Dependencies:** P7.01.
  - **Verification method:** Review the scope note against §5.3 and §9.1
    line by line; any §5.3 category without a verdict fails.
  - **Risks / failure modes:** Scope creep that publishes a contract for a
    protected invariant (e.g. a "custom authorizer" interface); a category
    silently supported without a conformance suite.

## P7.03 — License and dependency audit for the SDK stack (D4 follow-through)

- [ ] **P7.03 — License and dependency audit for the SDK stack (D4 follow-through)**
  - **Objective:** Prove every SDK dependency (Wasmtime, WIT tooling, gRPC
    stack, WASM component tooling, example-plugin dependencies) is
    license-compatible with the recommended Apache-2.0 core license, or
    record the incompatibility and the decision it forces.
  - **Report trace:** §25.2 (D4); §9.3; §22.2.
  - **Inputs / prerequisites:** Candidate dependency list from RESEARCH.md
    (Wasmtime ≥50.0.0 — see the CVE note in RESEARCH.md — `wit-bindgen`,
    `cargo-component`, `wasm-tools`, Tonic/Prost, Python worker deps); D4
    status.
  - **Steps:**
    1. Enumerate every new direct and transitive dependency with license
       (SPDX) and source; include the WASM toolchain and the Python gRPC
       worker template dependencies.
    2. Flag copyleft/unknown items; record the alternative for each
       (replace, isolate out-of-process, escalate to legal).
    3. Record the audit as a table and add it to the release SBOM inputs
       (§9.3).
    4. Add a CI check that fails on a new dependency without a license
       entry (implemented in P7.27, specified here).
  - **Deliverable / output:** License-audit table + SBOM input list; D4
    status update if the audit closes or narrows D4.
  - **Acceptance criteria / exit check:** Every SDK dependency has a
    recorded license and a pass/flag verdict; zero unknown-license items;
    every flagged item has a named action and owner.
  - **Owner role:** Legal + engineering.
  - **Dependencies:** P7.02.
  - **Verification method:** Re-run the enumeration from the lockfiles once
    code exists and diff against the audit table.
  - **Risks / failure modes:** A WASM toolchain component with an
    incompatible license discovered after the SDK ships; the Wasmtime
    version pinned below the CVE fix (see RESEARCH.md R7-C2).

## P7.04 — Capability-seam inventory: contract → provider → consumer map

- [ ] **P7.04 — Capability-seam inventory: contract → provider → consumer map**
  - **Objective:** Produce the complete three-role map (§5.2) for every
    seam in scope: contract definition, each provider, each consumer — with
    no consumer depending on a concrete provider.
  - **Report trace:** §5.2; §9.1; §6.2.
  - **Inputs / prerequisites:** P7.01–P7.02; §8.2 dependency rules.
  - **Steps:**
    1. For each contract, list: the trait/interface definition and its
       crate; every provider (built-in Rust, and planned WASM/gRPC);
       every consumer (application/domain module) and the exact import it
       uses.
    2. Verify by inspection and by build-graph query that no consumer
       imports a provider crate directly (e.g. Scheduling must not import
       a concrete HiGHS/Clarabel/CVXPY implementation).
    3. Record solver-class declarations per provider: HiGHS LP/QP/MIP
       (not SOCP); Clarabel convex conic incl. SOCP (not MISOCP); CVXPY
       modelling layer (capabilities depend on the selected solver); IPOPT
       local (no global guarantee) — §7.4, AGENTS.md rule 4.
    4. Publish the map as the SDK's seam index.
  - **Deliverable / output:** Seam map (contract → providers → consumers)
    with solver-class declarations.
  - **Acceptance criteria / exit check:** Every in-scope contract has at
    least one provider and one consumer listed; zero direct
    consumer→provider imports found; every optimizer provider declares its
    problem class, tolerances, and diagnostics per §7.4.
  - **Owner role:** Architect.
  - **Dependencies:** P7.02.
  - **Verification method:** Build-graph/import grep re-run; solver-class
    declarations checked against §7.4 and the solver's own documentation.
  - **Risks / failure modes:** A seam that exists only on paper; a provider
    whose declared problem class is wrong (e.g. HiGHS described as SOCP) —
    a solver-class conflation the repo forbids.

## P7.05 — Plugin manifest schema (versioned, validated, published)

- [ ] **P7.05 — Plugin manifest schema (versioned, validated, published)**
  - **Objective:** Define, version, and publish the plugin manifest schema,
    turning the §5.5 illustrative example into a validated schema — and
    record explicitly where it differs from the example.
  - **Report trace:** §5.5; §9.3; §10.5; §22.2.
  - **Inputs / prerequisites:** P7.02 scope; §5.5 example manifest.
  - **Steps:**
    1. Write the manifest JSON Schema (and an example YAML) covering:
       `apiVersion`, `kind`, `metadata` (id, version, displayName,
       publisher), `runtime` (`builtin-rust` | `wasm-component` |
       `grpc-worker`, entrypoint), `compatibility` (`flexcoreApi` range),
       `capabilities` (`provides`/`requires`), `permissions.requested`,
       `configuration.schema`, `health.readiness`, `resources`
       (maxMemoryMb, timeoutMs, plus fuel/time, concurrency, output-size,
       call-rate limits per §5.4 Class B).
    2. Version the schema (`flexcore.neryva.io/v1` or as decided; SemVer
       the schema itself) and publish it with the SDK.
    3. Write the validator: unknown fields rejected or explicitly warned
       (record the choice); missing required fields rejected; SemVer
       ranges parsed, not string-compared.
    4. Record every deliberate difference from the §5.5 example, with a
       reason; the example is illustrative, not a final schema (§5.5).
  - **Deliverable / output:** Manifest JSON Schema + validator + example
    manifests (valid and invalid) + differences-from-§5.5 note.
  - **Acceptance criteria / exit check:** Validator accepts all valid
    examples and rejects all invalid ones (missing id, bad SemVer range,
    unknown runtime kind, undeclared permission); schema version is
    recorded and published; differences from §5.5 are listed.
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.02.
  - **Verification method:** Run the validator against the example set;
    quote results.
  - **Risks / failure modes:** A manifest that validates but means nothing
    (permissions as free text, never enforced — see P7.12); schema drift
    between the published schema and the validator.

## P7.06 — Dependency resolution: missing, incompatible, cyclic, conflicting providers

- [ ] **P7.06 — Dependency resolution: missing, incompatible, cyclic, conflicting providers**
  - **Objective:** Implement dependency resolution that detects missing
    capabilities, version incompatibility, cycles, and conflicting providers
    **before activation** (§5.5).
  - **Report trace:** §5.5; §5.6; §9.3.
  - **Inputs / prerequisites:** P7.05 manifest schema + validator.
  - **Steps:**
    1. Implement the resolver: build the capability graph from manifests;
       resolve `provides`/`requires` against the compatibility matrix
       (§9.3) and `flexcoreApi` ranges.
    2. Detect and report, each with a distinct error: missing capability,
       version incompatibility, dependency cycle, two providers claiming
       the same capability without a resolution rule.
    3. Define the conflict-resolution rule in writing (explicit priority /
       explicit selection in the profile — never silent first-wins) and
       implement it.
    4. Test with crafted manifests for each failure mode plus a valid
       multi-provider graph.
  - **Deliverable / output:** Resolver implementation + conflict rule +
    failure-mode tests.
  - **Acceptance criteria / exit check:** Each of the four failure modes is
    detected before activation with a distinct, actionable error; a valid
    graph resolves; a conflict without an explicit rule is an error, never
    a silent choice.
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.05.
  - **Verification method:** Run the crafted-manifest suite; confirm no
    activation occurs for any failing case (activation log inspected).
  - **Risks / failure modes:** Silent first-wins masking a conflicting
    optimizer; a cycle detected only at runtime after partial activation.

## P7.07 — Plugin lifecycle state machine (with drain-before-replace)

- [ ] **P7.07 — Plugin lifecycle state machine (with drain-before-replace)**
  - **Objective:** Implement the §5.6 plugin lifecycle as an explicit state
    machine with preconditions, timeouts, and audit events — including the
    drain-before-replace rule for command-producing plugins.
  - **Report trace:** §5.6; §14.1 (by analogy); §6.4 (state machine pattern).
  - **Inputs / prerequisites:** P7.05, P7.06.
  - **Steps:**
    1. Implement states: `Discovered → Verified → Compatible →
       Configured → Started → Ready`, plus `Draining → Stopped`,
       `Rejected`, `Failed` — with the exact transitions in §5.6.
    2. Enforce: signature/provenance at `Verified`; manifest/API
       compatibility at `Compatible`; schema + secret-reference validity at
       `Configured`; dependency graph resolved at `Started`; health checks
       at `Ready`.
    3. Implement drain: on disable/upgrade, stop accepting new work, finish
       or cancel pending operations, record the version change, then
       replace. **No activation or upgrade in the middle of an active
       dispatch transaction; no hot-reload of safety or dispatch providers
       in the 1.x line** (§5.6).
    4. Audit every transition with plugin ID/version, actor, and reason.
  - **Deliverable / output:** Lifecycle implementation + transition tests +
    drain tests.
  - **Acceptance criteria / exit check:** Every §5.6 transition is tested
    including all `Rejected`/`Failed` exits; an upgrade attempted during
    an active dispatch transaction is refused (or queued until the
    transaction ends — record the chosen behaviour) and audited; a version
    change is recorded.
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.05, P7.06.
  - **Verification method:** State-machine property tests (§19.2(2)):
    random transition sequences never reach an undefined state.
  - **Risks / failure modes:** A plugin replaced mid-dispatch, leaving two
    versions' outputs in one transaction; a `Failed` plugin that never
    drains and blocks the registry.

## P7.08 — Built-in Rust provider registry (Class A)

- [ ] **P7.08 — Built-in Rust provider registry (Class A)**
  - **Objective:** Build the trusted built-in Rust provider registry:
    first-party providers compiled into the release, statically linked,
    registered at application composition time — replaceable at the contract
    level.
  - **Report trace:** §5.4 Class A; §5.2; §8.1 (`plugins/builtin/`);
    §6.4 (Factory/Registry pattern).
  - **Inputs / prerequisites:** P7.04 seam map; §8.1 target tree.
  - **Steps:**
    1. Implement the registry: only verified, compatible, allowlisted
       providers may be instantiated (§6.4); registration happens at the
       composition root (`apps/*`, `flexcore-kernel`), never via a global
       mutable registry or service locator (§6.4 "avoid").
    2. Register the existing built-in providers (asset simulator, HiGHS
       optimizer, market mock — §8.1 `plugins/builtin/`) behind their
       contracts; each declares its problem class and diagnostics (§9.2).
    3. Prove replaceability at the contract level: swap the registered
       optimizer provider in a test composition without touching
       application/domain code (this is the exit-gate rehearsal — see
       P7.28).
    4. Record the registry's allowlist policy: who approves a new built-in
       provider and how it is reviewed.
  - **Deliverable / output:** Registry implementation + registered
    built-ins + composition-swap test.
  - **Acceptance criteria / exit check:** All built-ins register through
    the registry (no direct construction in application code); the swap
    test passes; no global mutable registry exists (grep + inspection).
  - **Owner role:** Rust engineer.
  - **Dependencies:** P7.04, P7.07.
  - **Verification method:** Swap test run; grep for direct provider
    construction outside the composition root.
  - **Risks / failure modes:** A service locator creeping in; a built-in
    registered without its §9.2 diagnostics, making its results
    ineligible for operational consideration.

## P7.09 — WIT interface packages: versioned worlds for each contract

- [ ] **P7.09 — WIT interface packages: versioned worlds for each contract**
  - **Objective:** Define the WIT packages and versioned worlds for the
    v0.5.0 contracts, under `wit/`, with SemVer and a compatibility policy.
  - **Report trace:** §5.4 Class B; §8.1 (`wit/`); §9.1; §10.5; §22.2
    (WIT compatibility validation).
  - **Inputs / prerequisites:** P7.02 scope; P7.04 seam map; RESEARCH.md
    (WASI 0.2 baseline; WASI 0.3 not a release blocker).
  - **Steps:**
    1. Write one WIT package per contract family (e.g. optimizer, grid
       model, forecast, telemetry source), each with a versioned world;
       use `wasi:cli/command`-style worlds only where the contract needs
       them — do not import WASI interfaces a plugin does not need.
    2. Define the data vocabulary in WIT: canonical units (MW/MVAr/MVA/
       MWh), SOC [0,1], quality states, timestamps (UTC, event vs ingest),
       and the §9.2 result requirements (plugin/model/solver version,
       input snapshot ID, horizon, problem class, termination status,
       diagnostics, reproducibility metadata).
    3. Set the compatibility policy: breaking WIT changes get a new major
       world version; additive changes are recorded; `wit-bindgen`
       compatibility checks run in CI (P7.27).
    4. Record the WASI baseline decision: **WASI 0.2 / Component Model
       baseline for v0.5.0** (§5.4); WASI 0.3 is tracked, not required —
       see RESEARCH.md R7-C1.
  - **Deliverable / output:** WIT packages under `wit/` + compatibility
    policy + WASI-baseline record.
  - **Acceptance criteria / exit check:** Every in-scope contract has a
    WIT world; the WIT types carry units, quality, and §9.2 provenance
    fields (no bare floats without units/context); the baseline (WASI 0.2)
    is recorded with its re-evaluation trigger.
  - **Owner role:** Rust engineer (SDK) + architect.
  - **Dependencies:** P7.02, P7.04.
  - **Verification method:** `wit-bindgen`/component tooling validates
    the packages; a reviewer checks the WIT types against §9.2 field by
    field.
  - **Risks / failure modes:** WIT types that lose units/quality/provenance
    (a bare `f64` power value with no unit or sign convention); designing
    for WASI 0.3 and then discovering the toolchain is not stable (see
    RESEARCH.md).

## P7.10 — Wasmtime component host prototype (separate process)

- [ ] **P7.10 — Wasmtime component host prototype (separate process)**
  - **Objective:** Build the constrained WASM plugin host as a separate
    process (`apps/plugin-host/`) on Wasmtime's Component Model embedding
    API, running one example component behind a WIT world from P7.09.
  - **Report trace:** §5.4 Class B; §8.1 (`apps/plugin-host/`); §8.3;
    §7.3 (Wasmtime); §21.4 (WASM Plugin Host build form).
  - **Inputs / prerequisites:** P7.09 WIT worlds; RESEARCH.md (Wasmtime
    ≥50.0.0, CVE fixes; WASI 0.2 baseline).
  - **Steps:**
    1. Create `apps/plugin-host/` as a separate process (per §8.3: the WASM
       host is extracted for security isolation); wire it to the control
       plane over gRPC/Protobuf, not by in-process linking.
    2. Instantiate a component with Wasmtime's Component Model API; bind
       only the WIT imports the component's world declares — no ambient
       filesystem, network, database, or secret access (§5.4).
    3. Apply resource limits: memory, fuel/time, concurrency, output-size,
       and call-rate limits (§5.4).
    4. Run one example component end to end (e.g. a forecast or
       schedule-proposal component) and record its behaviour under each
       limit.
  - **Deliverable / output:** `apps/plugin-host/` prototype + example
    component + limit-behaviour record.
  - **Acceptance criteria / exit check:** Example component runs and
    returns a §9.2-conformant result; a component that exceeds a limit is
    terminated cleanly with an audit event (not a host crash); no ambient
    access is available to the component (tested in P7.13).
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.09, P7.03.
  - **Verification method:** Run the prototype; attempt to exceed each
    limit and record the host's response.
  - **Risks / failure modes:** Pinning a Wasmtime version below the
    October 2026 CVE fixes (see RESEARCH.md R7-C2) — the prototype must
    pin ≥50.0.0 or a fixed backport; a host crash on guest misbehaviour
    (the sandbox must contain the fault).

## P7.11 — Host capability broker: scoped capabilities, no ambient access

- [ ] **P7.11 — Host capability broker: scoped capabilities, no ambient access**
  - **Objective:** Implement the host-side capability broker that grants a
    component only the specific scoped capabilities its manifest requests
    and the profile allows — and nothing else.
  - **Report trace:** §5.4 Class B; §5.7; §17.3; §17.4.
  - **Inputs / prerequisites:** P7.05 manifest permissions; P7.10 host.
  - **Steps:**
    1. Define the capability vocabulary from §5.7 (e.g.
       `telemetry.read`, `schedule.propose`, `forecast.publish`), each
       scoped to tenant/site/asset.
    2. Implement the broker: every host function a component can call goes
       through the broker; the broker checks manifest permissions +
       profile + tenant scope before dispatching.
    3. Enforce the never-granted set: `dispatch.authorize`,
       `secret.read` (secret references resolved by host/adapter, never
       handed to the plugin), `network.any` (prohibited by default),
       direct device-protocol access (§5.7).
    4. Keep the `dispatch.validate.request` boundary explicit: the broker
       may forward a plugin's *request* for validation to the protected
       validator, but the request path never returns a validation verdict
       to the plugin and never substitutes for the protected validation
       gate (§5.7) — a plugin that receives a validation outcome and
       treats it as authorization has crossed the boundary.
    5. Log every denied capability call with plugin ID, capability, and
       scope.
  - **Deliverable / output:** Capability broker + capability vocabulary +
    denial tests.
  - **Acceptance criteria / exit check:** A component requesting an
    ungranted capability is denied with an audit event; a component with
    only `telemetry.read` cannot call `schedule.propose`; no capability
    in the never-granted set is grantable through any manifest or profile.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P7.05, P7.10.
  - **Verification method:** Negative tests per capability; attempt to
    grant `dispatch.authorize` via a crafted manifest and confirm the
    validator/broker refuses.
  - **Risks / failure modes:** Capabilities enforced only in documentation
    while the host functions are directly callable; tenant scope checked
    at the edge of the call but not on the data returned.

## P7.12 — Permission enforcement: manifest permissions are enforced, not decorative

- [ ] **P7.12 — Permission enforcement: manifest permissions are enforced, not decorative**
  - **Objective:** Prove the §5.7 permission table is enforced at runtime
    for every plugin class, including the "candidate only" rule for
    `schedule.propose` / `dispatch.propose`.
  - **Report trace:** §5.7; §9.1; §13.4; §17.5.
  - **Inputs / prerequisites:** P7.11 broker; P7.08 registry; P7.20 gRPC
    template.
  - **Steps:**
    1. Write the enforcement matrix: every §5.7 row × every plugin class
       (A/B/C) → allowed/denied/candidate-only, with the enforcing code
       location named for each cell.
    2. Implement the candidate-only rule: a plugin's proposed schedule or
       dispatch is a candidate; it enters the protected validation →
       feasibility → authorization path and gains no authority from being
       proposed (§13.4: research plugins return proposals, never an
       authorization token).
    3. Test privilege escalation: a plugin that tries to call
       `CommandAuthorizer`, `DispatchGateway`, or a protected endpoint
       directly, and a plugin that tries to mutate another module's
       database tables (§6.4).
    4. Test tenant-scope enforcement: a plugin scoped to site A reading
       site B's telemetry is denied and audited.
  - **Deliverable / output:** Enforcement matrix + escalation tests.
  - **Acceptance criteria / exit check:** Every §5.7 row is enforced for
    every plugin class (matrix fully filled with code locations); all
    escalation attempts fail and are audited; a proposed schedule never
    becomes an issued command without the protected path.
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.11, P7.08.
  - **Verification method:** Run the escalation suite; trace one proposed
    schedule from a plugin through validation/feasibility/authorization
    and confirm each gate ran.
  - **Risks / failure modes:** Enforcement in the WASM host but not in the
    gRPC worker path (or vice versa); a "candidate" that skips feasibility
    because it arrived through a trusted built-in.

## P7.13 — Sandbox negative tests: escape, exhaustion, and ambient-access attempts

- [ ] **P7.13 — Sandbox negative tests: escape, exhaustion, and ambient-access attempts**
  - **Objective:** Attempt to break the sandbox from inside a component —
    filesystem, network, environment, secrets, resource exhaustion, and
    host-function abuse — and record that each attempt fails safely.
  - **Report trace:** §5.4 Class B; §17.4; §19.2(7); §7.3 ([R4] Wasmtime
    security).
  - **Inputs / prerequisites:** P7.10 host; P7.11 broker.
  - **Steps:**
    1. Write hostile test components: one that opens files, one that
       opens sockets, one that reads environment variables, one that
       allocates unboundedly, one that loops forever, one that returns a
       multi-GB output, one that calls host functions at abusive rates.
    2. Run each against the host; record the outcome (denied, terminated
       by limit, host unaffected).
    3. Record the Wasmtime version and the sandboxing caveat from §5.4:
       Wasmtime provides sandboxing mechanisms, but sandboxing must still
       be combined with host capability policy, resource limits,
       signatures, and isolation ([R4]).
    4. Re-check the GHSA advisory pages for CVE-2026-115889 /
       CVE-2026-115893 / CVE-2026-115384 at implementation time and record
       the advisory IDs and the check date; confirm the pinned Wasmtime
       version carries all three fixes (RESEARCH.md §7 task 1).
    5. Fix every case where the host is affected, the component escapes a
       limit, or an ambient resource is reachable; re-test.
  - **Deliverable / output:** Sandbox negative-test suite + results record.
  - **Acceptance criteria / exit check:** All hostile components fail
    safely; the host process survives every case; no ambient resource is
    reachable; the Wasmtime version is recorded and is ≥ the CVE-fixed
    release (see RESEARCH.md).
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.10, P7.11.
  - **Verification method:** Independent re-run of the hostile suite by a
    second engineer on a clean build.
  - **Risks / failure modes:** Testing only polite components; a resource
    limit that exists in config but is never applied to the component
    instance.

## P7.14 — Plugin signing, provenance, and SBOM (supply chain)

- [ ] **P7.14 — Plugin signing, provenance, and SBOM (supply chain)**
  - **Objective:** Require signatures/provenance for plugin installation,
    generate SBOMs for plugin packages, pin plugin dependencies, and retain
    rollback artifacts — with no plugin able to self-upgrade or fetch
    executable code at runtime.
  - **Report trace:** §9.3; §5.6; §17.3; §22.2.
  - **Inputs / prerequisites:** P7.05 manifest; P7.07 lifecycle.
  - **Steps:**
    1. Define the signing scheme: which keys sign plugin packages, how
       signatures are verified at `Discovered → Verified`, and how
       revocation is handled.
    2. Implement verification in the lifecycle: an unsigned or
       bad-signature package is `Rejected` before any compatibility check.
    3. Generate an SBOM per plugin package and per release; pin plugin
       dependencies; retain the previous version's artifact for rollback.
    4. Implement the prohibitions: no self-upgrade, no runtime fetching of
       executable code (§9.3) — test both with a hostile package.
  - **Deliverable / output:** Signing/verification implementation + SBOM
    generation + rollback-artifact retention + negative tests.
  - **Acceptance criteria / exit check:** Unsigned package rejected;
    tampered package rejected; SBOM exists per package; rollback to the
    previous version works; self-upgrade and runtime code-fetch attempts
    fail.
  - **Owner role:** Security + DevOps.
  - **Dependencies:** P7.05, P7.07.
  - **Verification method:** Tamper with a signed package and attempt
    install; attempt self-upgrade from inside a component.
  - **Risks / failure modes:** Signature verification that warns instead of
    rejecting; SBOM generated but never checked against the license audit
    (P7.03).

## P7.15 — Composition profiles and bundles (`sim-lab`, `vpp-operator`, `site-edge`, research)

- [ ] **P7.15 — Composition profiles and bundles (`sim-lab`, `vpp-operator`, `site-edge`, research)**
  - **Objective:** Implement named runtime profiles composed from ordered
    bundles per §5.8, as explicit, reviewable compositions that never
    silently enable a control-capable plugin.
  - **Report trace:** §5.8; §5.1; §21.4.
  - **Inputs / prerequisites:** P7.06 resolver; P7.08 registry; P6.29
    `site-edge` input.
  - **Steps:**
    1. Implement profiles as data (not code): `sim-lab` (synthetic DERs,
       feeder simulator, mock market, deterministic optimizer, replay
       tools, no device writes); `vpp-operator` (registry, telemetry,
       flexibility, scheduler, audit, console, selected site connectors);
       `site-edge` (edge agent, selected protocol adapters, local
       buffering, outbound identity, limited command receiver);
       `research-gnn` and `research-marl` (grid model provider, worker,
       benchmark suite, shadow logger, **no dispatch authority**).
    2. Enforce: a profile must not silently enable a control-capable
       plugin; enabling one requires an explicit, reviewed profile change.
    3. Validate each profile at load: resolve its bundle graph (P7.06),
       verify every plugin's lifecycle state, and refuse to start a
       profile with an unresolved or incompatible bundle.
    4. Ship v0.5.0 with the profiles that are ready; record which profiles
       are deferred and why (v0.1.0 shipped only `sim-lab` with static
       built-ins — §5.8).
  - **Deliverable / output:** Profile definitions + loader/validator +
    per-profile composition record.
  - **Acceptance criteria / exit check:** Each profile loads, resolves,
    and starts its bundles; a profile containing a control-capable plugin
    without an explicit declaration is refused; `research-gnn` /
    `research-marl` profiles grant no dispatch authority (verified by
    attempting a dispatch from within them).
  - **Owner role:** Architect.
  - **Dependencies:** P7.06, P7.08.
  - **Verification method:** Load each profile from its definition file
    alone; attempt the silent-enable case and confirm refusal.
  - **Risks / failure modes:** A profile that is really a code path with
    hidden defaults; research profiles that inherit `vpp-operator`'s
    dispatch capability by accident.

## P7.16 — Profile switching and safe re-composition (no silent control changes)

- [ ] **P7.16 — Profile switching and safe re-composition (no silent control changes)**
  - **Objective:** Define and implement safe profile switching: drain,
    verify, switch, and audit — with control-authority mode never changed
    by a profile switch.
  - **Report trace:** §5.6; §5.8; §21.4; §14.1.
  - **Inputs / prerequisites:** P7.07 lifecycle; P7.15 profiles.
  - **Steps:**
    1. Specify the switch procedure: drain affected plugins (P7.07),
       resolve the new profile (P7.06), verify compatibility, switch,
       record the version change, audit.
    2. Enforce: control-authority mode is a deployment/per-site setting
       (§21.4), not a profile property; a profile switch cannot raise it.
    3. Test: switch `sim-lab` → `vpp-operator` and back; switch while a
       dispatch transaction is active (must refuse or defer per P7.07);
       switch to a profile with an incompatible plugin (must refuse).
    4. Record the rollback: switching back restores the previous
       composition, with its own audit event.
  - **Deliverable / output:** Switch procedure + implementation + tests.
  - **Acceptance criteria / exit check:** All switch tests pass; no switch
    changes control-authority mode; an in-flight dispatch blocks or defers
    the switch; every switch is audited with before/after composition.
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.07, P7.15.
  - **Verification method:** Run the switch suite; inspect the audit log
    for before/after composition records.
  - **Risks / failure modes:** A switch that drops the audit plugin and
    then cannot record the switch; a profile switch used to smuggle in a
    control-capable plugin.

## P7.17 — Python gRPC worker template (Class C)

- [ ] **P7.17 — Python gRPC worker template (Class C)**
  - **Objective:** Build the out-of-process Python gRPC worker template
    that Class C plugins are written from, with Protobuf contracts,
    lifecycle, health, and isolation — so Python numerical/ML code never
    runs in the Rust control plane.
  - **Report trace:** §5.4 Class C; §7.2; §7.4; §8.1 (`python/workers/`);
    §15.1.
  - **Inputs / prerequisites:** P7.09 WIT worlds (for the data vocabulary);
    §10.5 Protobuf governance; Phase 1 gRPC setup.
  - **Steps:**
    1. Write the template: a Python worker implementing one contract
       (e.g. `Optimizer` via the Python optimization stack, or
       `ForecastProvider`) over gRPC/Protobuf, with config, logging,
       health/readiness endpoints, and graceful shutdown.
    2. Define the worker contract: request/response Protobuf messages
       carrying the §9.2 result requirements (versions, snapshot ID,
       horizon, problem class, termination status, diagnostics,
       reproducibility metadata).
    3. Isolate: the worker runs in its own container/process with CPU/
       memory limits; a worker crash does not take down the control plane
       and is reported as a plugin `Failed` transition (§5.6).
    4. Apply `buf lint` and `buf breaking` to the worker's Protobuf
       schemas in CI (§10.5).
  - **Deliverable / output:** Worker template + example worker + Protobuf
    schemas + isolation tests.
  - **Acceptance criteria / exit check:** Example worker runs out-of-process,
    serves the contract, returns §9.2-conformant results, and its crash
    is contained and reported; `buf lint` passes; a breaking schema change
    is detected by `buf breaking`.
  - **Owner role:** Python engineer.
  - **Dependencies:** P7.09.
  - **Verification method:** Kill the example worker mid-request and
    confirm the control plane survives and reports `Failed`; run
    `buf lint` / `buf breaking`.
  - **Risks / failure modes:** A second general-purpose Python backend
    growing inside the worker (§7.2 forbids this); Protobuf schemas
    drifting from the WIT data vocabulary so the same concept has two
    shapes.

## P7.18 — Worker supervision: timeouts, retries, crash isolation, and backpressure

- [ ] **P7.18 — Worker supervision: timeouts, retries, crash isolation, and backpressure**
  - **Objective:** Supervise Class C workers with explicit timeouts,
    bounded retries, crash isolation, and backpressure — so a slow or dead
    worker degrades the schedule, never the platform.
  - **Report trace:** §5.4 Class C; §19.2(6)(8); §6.4 (outbox/saga patterns).
  - **Inputs / prerequisites:** P7.17 template.
  - **Steps:**
    1. Define per-contract timeouts (from manifest `resources.timeoutMs`
       and contract defaults); a timeout produces a recorded failure with
       diagnostics, not a hung schedule.
    2. Define the retry policy: bounded retries with backoff, idempotency
       keys on every request, no retry of a non-idempotent dispatch-adjacent
       call.
    3. Implement backpressure: when workers are saturated, new work is
       queued with a visible depth metric or rejected with a recorded
       reason — never accepted and silently dropped.
    4. Test: worker timeout, worker crash mid-request, worker restart,
       worker saturation.
  - **Deliverable / output:** Supervision implementation + policy + tests.
  - **Acceptance criteria / exit check:** Each failure mode behaves as
    specified with an audit/diagnostic record; no hung schedule; no
    duplicate side effect from a retry (idempotency demonstrated).
  - **Owner role:** Rust engineer.
  - **Dependencies:** P7.17.
  - **Verification method:** Fault-injection run against the example
    worker; inspect queue-depth metrics under saturation.
  - **Risks / failure modes:** Unbounded retries amplifying a failure;
    a retry re-issuing a side effect; saturation silently dropping work.

## P7.19 — Plugin configuration: schema validation and secret references (never secrets in config)

- [ ] **P7.19 — Plugin configuration: schema validation and secret references (never secrets in config)**
  - **Objective:** Validate every plugin's configuration against its
    declared JSON Schema at `Configured` time, and resolve secrets only by
    reference through the host/adapter — never as values in config files.
  - **Report trace:** §5.5; §5.6; §5.7 (`secret.read` never granted);
    §17.3.
  - **Inputs / prerequisites:** P7.05 manifest (`configuration.schema`);
    P7.07 lifecycle.
  - **Steps:**
    1. Implement config validation: schema fetched from the manifest,
       config validated before `Configured`; invalid config → `Rejected`
       with the failing fields listed.
    2. Implement secret references: config may name a secret reference;
       the host resolves it from the secrets manager and hands the
       adapter — never the plugin — the value; the plugin receives only a
       handle or nothing.
    3. Test: config with a literal secret value is rejected (or the value
       is refused and the attempt audited — record the chosen behaviour);
       config referencing a nonexistent secret is rejected at
       `Configured`, not at first use.
    4. Confirm no secret appears in logs, error messages, or stored config
       (P7.23 secret-exposure test covers this end to end).
  - **Deliverable / output:** Config validation + secret-reference
    resolution + tests.
  - **Acceptance criteria / exit check:** Invalid config rejected with
    field-level reasons; literal-secret config rejected/audited; missing
    secret reference rejected at `Configured`; secrets never appear in
    logs or stored config.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P7.05, P7.07.
  - **Verification method:** Submit the hostile configs and record the
    rejections; grep logs and stored config for the test secret value.
  - **Risks / failure modes:** A secret that passes validation because the
    schema allows free-form strings; a secret resolved once and then
    cached in plaintext in plugin memory.

## P7.20 — Plugin health, readiness, and observability

- [ ] **P7.20 — Plugin health, readiness, and observability**
  - **Objective:** Give every plugin health/readiness signals and wire
    plugin telemetry into the platform's observability (§21.3) with
    correlation IDs — without leaking secrets or customer data into logs.
  - **Report trace:** §5.5 (`health.readiness`); §5.6; §21.3.
  - **Inputs / prerequisites:** P7.07 lifecycle; Phase 1 tracing/metrics.
  - **Steps:**
    1. Define health semantics: `readiness: required` vs optional; health
       check contract per plugin class (WASM component, gRPC worker,
       built-in).
    2. Implement health aggregation: a `Failed` health check moves the
       plugin to `Failed` (§5.6) and surfaces in the console's Plugins &
       Integrations workspace (§20.1).
    3. Correlate every plugin invocation with tenant/site, schedule/
       command IDs, plugin/solver/model versions, and trace ID (§21.3).
    4. Add metrics: invocation count, duration, timeout/failure rate, and
       resource use per plugin; record solver duration/status for
       optimizer plugins.
  - **Deliverable / output:** Health/readiness implementation + console
    surfacing + metrics.
  - **Acceptance criteria / exit check:** An unhealthy plugin shows as
    failed in the console within the stated detection window; invocations
    are traceable by plugin version and trace ID; no secret or unnecessary
    personal/customer data appears in logs.
  - **Owner role:** Rust engineer.
  - **Dependencies:** P7.07, P7.17.
  - **Verification method:** Fail a plugin's health check in the lab and
    time the console transition; inspect a trace for the required
    correlation fields.
  - **Risks / failure modes:** Health reported as healthy by default when
    the check never ran; logs carrying plugin config values that include
    secret references.

## P7.21 — Plugin conformance suite (contract tests every plugin must pass)

- [ ] **P7.21 — Plugin conformance suite (contract tests every plugin must pass)**
  - **Objective:** Build the conformance suite (§19.2(3)) that every plugin
    passes regardless of class: the same interface suite for every adapter
    and plugin, run in CI.
  - **Report trace:** §19.2(3); §9.3; §22.2.
  - **Inputs / prerequisites:** P7.09 WIT worlds; P7.17 worker template;
    P7.08 registry.
  - **Steps:**
    1. Write contract tests per interface: valid inputs, invalid inputs,
       boundary values (SOC 0/1, P sign, zero-length horizons), error
       shapes, and §9.2 result completeness (a result without provenance
       and diagnostics fails).
    2. Run the same suite against a built-in provider, a WASM component,
       and a gRPC worker implementing the same contract; record per-class
       results.
    3. Add the suite to CI; a plugin package cannot be published/activated
       without a passing suite run recorded against its version.
    4. Include the Phase 6 adapter conformance approach as the model for
       protocol adapters (cross-reference P6.19, do not duplicate it).
  - **Deliverable / output:** Conformance suite in CI + per-class run
    records.
  - **Acceptance criteria / exit check:** Suite runs in CI and passes for
    all three classes on the same contract; a plugin returning a result
    without §9.2 provenance/diagnostics fails the suite; suite version is
    recorded with each run.
  - **Owner role:** Rust engineer (SDK) + Python engineer.
  - **Dependencies:** P7.08, P7.09, P7.17.
  - **Verification method:** CI run quoted; submit a deliberately
    non-conformant plugin (missing diagnostics) and confirm suite failure.
  - **Risks / failure modes:** A suite that only tests the happy path; a
    class-specific shortcut (e.g. WASM components exempted from a test the
    built-ins must pass).

## P7.22 — Swap test: replace one optimizer and one grid provider without touching application/domain code

- [ ] **P7.22 — Swap test: replace one optimizer and one grid provider without touching application/domain code**
  - **Objective:** Execute the §23 Stage 7 exit gate's first half as a
    witnessed test: swap one optimizer provider and one grid-model provider
    with zero changes to application or domain code.
  - **Report trace:** §23 Stage 7 (exit gate); §5.2; §12.3–§12.4.
  - **Inputs / prerequisites:** P7.08 registry; P7.15 profiles; Phase 3
    scheduler; Phase 4 grid provider.
  - **Steps:**
    1. Baseline: run the §24 slice scenario with the default optimizer
       (e.g. HiGHS LP/MILP) and default grid provider (e.g. pandapower);
       record schedule, feasibility result, and timings.
    2. Declare the equivalence criteria *before* either swap: the same
       feasibility verdict, the same schedule objective within a
       pre-recorded tolerance (record the tolerance and its basis), and
       the same §9.2 result completeness — timing is compared as a
       measurement, not as an equivalence criterion.
    3. Swap the optimizer provider (e.g. HiGHS → Clarabel on a convex
       conic formulation, or → the Python CVXPY worker) by profile/
       registry change only; re-run the same scenario; record the result.
       Note the solver-class discipline: the swap is valid only where the
       formulation fits the new solver's class (§7.4) — record the
       formulation change, if any, as a problem-definition change, not a
       code change.
    4. Swap the grid-model provider (e.g. pandapower → OpenDSS or
       GridLAB-D adapter, per availability) the same way; re-run; record.
    5. Diff the application/domain code before/after: the diff must be
       empty (only profile/registry/composition changed).
  - **Deliverable / output:** Swap-test record (baseline + two swaps,
    results, timings, code diff).
  - **Acceptance criteria / exit check:** Both swaps run the same scenario
    to a validated result; `git diff` over application/domain code is
    empty for both swaps; solver-class declarations are correct for each
    provider used; the pre-declared equivalence criteria from step 2 are
    met for both swaps.
  - **Owner role:** Architect + Rust engineer.
  - **Dependencies:** P7.08, P7.15, P7.21.
  - **Verification method:** Independent witness re-runs the swap from the
    record; `git diff` inspected.
  - **Risks / failure modes:** A "swap" that actually edits the scheduling
    use case; a Clarabel swap attempted on a MILP formulation (class
    mismatch presented as a successful swap); a grid-provider swap that
    changes the model fidelity without recording it (D3).

## P7.23 — Untrusted-plugin negative test: ungranted capabilities are unreachable

- [ ] **P7.23 — Untrusted-plugin negative test: ungranted capabilities are unreachable**
  - **Objective:** Execute the §23 Stage 7 exit gate's second half as a
    witnessed test: an untrusted plugin cannot access ungranted
    capabilities — demonstrated by attack, not by code review alone.
  - **Report trace:** §23 Stage 7 (exit gate); §5.7; §17.4; §19.2(7).
  - **Inputs / prerequisites:** P7.11 broker; P7.12 matrix; P7.13 sandbox
    tests; P7.17 worker template.
  - **Steps:**
    1. Build an untrusted test plugin (WASM component and, separately, a
       gRPC worker) that attempts: reading another tenant's telemetry,
       proposing a dispatch as if authorized, calling the authorizer,
       issuing a command, reading a secret, opening the network, writing
       to another module's tables. Run the same attempts from inside the
       `research-gnn` / `research-marl` profiles (P7.15) as well as from a
       standalone plugin context, so the P7.15 no-dispatch verification is
       bound to this test run.
    2. Run each attempt; record denial + audit for every one.
    3. Attempt the same from a built-in provider context (to catch the
       "trusted class bypasses enforcement" failure mode in P7.12).
    4. Record the results as the exit-gate evidence for this half of the
       gate.
  - **Deliverable / output:** Negative-test record (attempt → outcome →
    audit reference) for both WASM and gRPC classes.
  - **Acceptance criteria / exit check:** Every attempt is denied and
    audited; zero attempts succeed; the built-in-context attempts are
    also denied where the capability is ungranted.
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.11, P7.12, P7.13, P7.17.
  - **Verification method:** Independent re-run of the attack plugin by a
    second engineer.
  - **Risks / failure modes:** A test that only checks the broker's
    configuration instead of actually attempting the call; a gRPC worker
    that bypasses the broker because it talks to the database directly.

## P7.24 — SDK documentation and examples (including a deliberately bad example)

- [ ] **P7.24 — SDK documentation and examples (including a deliberately bad example)**
  - **Objective:** Publish the plugin SDK documentation and working
    examples — including one example that is deliberately wrong, so the
    conformance suite's rejection is demonstrated, not just described.
  - **Report trace:** §8.1 (`docs/plugin-sdk/`, `plugins/wasm-examples/`,
    `plugins/manifests/`); §22.2.
  - **Inputs / prerequisites:** P7.05, P7.09, P7.17, P7.21.
  - **Steps:**
    1. Write the SDK guide: concepts (seams, classes, lifecycle,
       capabilities), manifest authoring, WIT authoring, gRPC worker
       authoring, testing against the conformance suite, publishing
       (signing, SBOM).
    2. Ship working examples: one WASM component (e.g. forecast provider)
       and one Python gRPC worker (e.g. optimizer), each with a manifest,
       tests, and a README.
    3. Ship one deliberately non-conformant example (missing §9.2
       diagnostics, or requesting `dispatch.authorize`) with a README
       explaining exactly why the suite rejects it — and the recorded
       rejection output.
    4. Run `python tools/verify_docs.py` after any doc change.
  - **Deliverable / output:** SDK docs under `docs/plugin-sdk/` + examples
    under `plugins/` + rejection record for the bad example.
  - **Acceptance criteria / exit check:** A new engineer builds, tests,
    and activates an example plugin from the docs alone; the bad example
    is rejected by the suite with the documented reason; `verify_docs.py`
    passes.
  - **Owner role:** Technical writer + SDK engineer.
  - **Dependencies:** P7.05, P7.09, P7.17, P7.21.
  - **Verification method:** Clean-machine walkthrough by an engineer who
    did not write the docs.
  - **Risks / failure modes:** Docs describing a planned SDK instead of the
    built one; examples that only work on the author's machine; the bad
    example accidentally passing.

## P7.25 — Operator console: Plugins & Integrations workspace

- [ ] **P7.25 — Operator console: Plugins & Integrations workspace**
  - **Objective:** Show installed plugins, capabilities, permissions,
    versions, health, and configuration in the console (§20.1 item 10),
    with dangerous actions requiring explicit confirmation.
  - **Report trace:** §20.1; §20.2; §5.6.
  - **Inputs / prerequisites:** P7.20 health; P7.07 lifecycle; Phase 5
    console.
  - **Steps:**
    1. Build/extend the Plugins & Integrations workspace: plugin list with
       ID/version/publisher, capabilities provided/required, permissions
       granted, lifecycle state, health, config summary (never secret
       values).
    2. Show profile composition: which profile is active, which bundles it
       contains, and the last composition change with actor and audit ref.
    3. Require explicit confirmation for high-impact actions (activate,
       deactivate, upgrade a plugin; switch a profile).
    4. Distinguish candidate/proposed outputs from authorized/executed
       actions wherever a plugin's output is displayed (§20.2).
  - **Deliverable / output:** Console changes + UI test record.
  - **Acceptance criteria / exit check:** An operator can determine every
    plugin's permissions and state from the console; no secret value is
    displayed; high-impact actions require confirmation; plugin-proposed
    values are never displayed as executed.
  - **Owner role:** Frontend engineer.
  - **Dependencies:** P7.20, P7.07.
  - **Verification method:** Operator walkthrough against the lab; confirm
    each state and permission is visible.
  - **Risks / failure modes:** Permissions displayed from the manifest
    while the broker enforces something different (display vs enforcement
    drift — cross-check with P7.12).

## P7.26 — Performance and resource validation of the plugin host and workers

- [ ] **P7.26 — Performance and resource validation of the plugin host and workers**
  - **Objective:** Measure the plugin host's and workers' resource use and
    latency on the §24 slice scenario and record limits — no invented
    numbers.
  - **Report trace:** §19.2(8); §19.4.
  - **Inputs / prerequisites:** P7.10 host; P7.17 worker; P7.22 swap test.
  - **Steps:**
    1. Define the load profile: invocations per schedule run, component
       and worker counts, payload sizes.
    2. Measure: WASM call latency (p95/p99), gRPC worker latency
       (p95/p99), host memory/CPU, worker memory/CPU, and the overhead of
       the capability broker.
    3. Measure the failure case: a component hitting its fuel/time limit
       and a worker hitting its timeout — record the platform's behaviour
       and recovery time.
    4. Record results as measured values with versions and conditions; do
       not extrapolate beyond the tested profile.
  - **Deliverable / output:** Plugin performance report (measured values,
    versions, conditions, limits).
  - **Acceptance criteria / exit check:** Measurements exist for the stated
    profile for both WASM and gRPC classes; broker overhead is measured,
    not assumed; limit-hit behaviour is recorded.
  - **Owner role:** Rust engineer.
  - **Dependencies:** P7.10, P7.17.
  - **Verification method:** Re-run the load test and compare against the
    recorded numbers within stated tolerance.
  - **Risks / failure modes:** Host-process numbers presented as
    per-component numbers; broker overhead unmeasured and later blamed for
    a scheduling deadline miss.

## P7.27 — CI gates: fmt/clippy/tests, Buf, WIT validation, manifest validation, SBOM, scanning

- [ ] **P7.27 — CI gates: fmt/clippy/tests, Buf, WIT validation, manifest validation, SBOM, scanning**
  - **Objective:** Wire every §22.2 CI gate to the SDK work and add the
    plugin-specific gates: manifest validation, WIT compatibility
    validation, Buf lint/breaking, SBOM, and the license check from P7.03.
  - **Report trace:** §22.2; §10.5; §9.3.
  - **Inputs / prerequisites:** P7.05, P7.09, P7.17, P7.21.
  - **Steps:**
    1. Extend CI: `cargo fmt --check`; `cargo clippy --workspace
       --all-targets -- -D warnings`; `cargo test --workspace`; Python
       Ruff + type checks + pytest; frontend lint/type-check/test/build;
       Buf lint + breaking; plugin manifest validation; WIT compatibility
       validation; migration/schema tests; container/dependency scanning;
       SBOM generation; license/security review; scenario regression suite.
    2. Add the plugin conformance suite (P7.21) as a CI job.
    3. Add the license gate: a new dependency without a license entry fails
       CI (specified in P7.03; demonstrate once on a scratch branch).
    4. Record a green CI run on the phase branch.
  - **Deliverable / output:** CI configuration + green run record.
  - **Acceptance criteria / exit check:** All §22.2 gates run on the new
    code and pass; manifest and WIT validation run in CI; a breaking WIT
    change without a version bump fails CI (demonstrated once); the
    license gate fails CI by construction (demonstrated once).
  - **Owner role:** DevOps.
  - **Dependencies:** P7.05, P7.09, P7.17, P7.21, P7.03.
  - **Verification method:** Inspect the CI run; run the two negative
    demonstrations on scratch branches.
  - **Risks / failure modes:** CI that validates manifests but never the
    WIT worlds (or vice versa); a WIT breaking change merged because the
    check ran on the wrong path.

## P7.28 — Exit-gate run: swap + untrusted-plugin proofs, witnessed

- [ ] **P7.28 — Exit-gate run: swap + untrusted-plugin proofs, witnessed**
  - **Objective:** Execute the §23 Stage 7 exit gate as a single witnessed
    run, combining the P7.22 swap proof and the P7.23 untrusted-plugin
    proof, and record the gate decision.
  - **Report trace:** §23 Stage 7 (exit gate).
  - **Inputs / prerequisites:** P7.22, P7.23; CHECKLIST.md.
  - **Steps:**
    1. Write the run book: exact versions (FlexCore, Wasmtime, WIT worlds,
       worker template, plugin versions), scenario, and evidence to capture.
    2. Run the swap proof and the untrusted-plugin proof back to back from
       the run book; capture the code diff, the results, and the audit log.
    3. A witness who did not build the SDK re-runs the run book on a clean
       build on a clean machine, from the evidence pack alone, and confirms
       reproduction.
    4. Record the gate decision (pass/fail per CHECKLIST.md item) with the
       witness's name and date.
  - **Deliverable / output:** Exit-gate run book + evidence pack + signed
    gate record.
  - **Acceptance criteria / exit check:** Both halves of the gate pass as
    specified in P7.22/P7.23; the witness reproduces both; every
    CHECKLIST.md item has evidence or is explicitly failed.
  - **Owner role:** Architect + security reviewer.
  - **Dependencies:** P7.22, P7.23.
  - **Verification method:** Witness reproduction (step 3) is itself the
    verification.
  - **Risks / failure modes:** A gate passed on a run nobody can reproduce;
    the two halves of the gate run on different builds.

## P7.29 — Security testing: supply chain, escalation, and host compromise scenarios

- [ ] **P7.29 — Security testing: supply chain, escalation, and host compromise scenarios**
  - **Objective:** Run the §19.2(7) security layer against the plugin
    surface — supply-chain compromise, privilege escalation, tenant
    leakage, secrets exposure, host compromise via a malicious plugin —
    and record every finding.
  - **Report trace:** §19.2(7); §17.4; §9.3.
  - **Inputs / prerequisites:** P7.13, P7.14, P7.23.
  - **Steps:**
    1. Test a compromised-plugin scenario: a signed plugin whose behaviour
       turns malicious after activation (excessive calls, wrong-tenant
       reads, oversized outputs) — the host must contain, limit, and
       report it, and the operator must be able to drain and remove it.
    2. Test supply-chain scenarios: tampered package, revoked signature,
       dependency confusion in a plugin's pinned dependencies.
    3. Test tenant leakage across plugin classes and the audit of denials.
    4. Record findings in a dated security-test report; every high/critical
       finding blocks the exit gate until fixed and re-tested.
  - **Deliverable / output:** Security-test report with findings,
    severities, fixes, re-test evidence.
  - **Acceptance criteria / exit check:** All §19.2(7) categories exercised
    against the plugin surface; zero open high/critical findings; a
    malicious plugin is contained and removable.
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.13, P7.14, P7.23.
  - **Verification method:** Independent re-run of the attack cases by a
    second engineer; findings tracked to closure.
  - **Risks / failure modes:** Security tests that never use a genuinely
    malicious component; a plugin that cannot be removed because its
    lifecycle has no drain path for the `Failed` state.

## P7.30 — Failure-mode drill (plugin host, workers, registry)

- [ ] **P7.30 — Failure-mode drill (plugin host, workers, registry)**
  - **Objective:** Run a combined drill over the §19.2(6) fault set against
    the plugin host, gRPC workers, and registry, and record recovery time
    and data loss per fault.
  - **Report trace:** §19.2(6); §21.2.
  - **Inputs / prerequisites:** P7.10, P7.17, P7.18.
  - **Steps:**
    1. Drill each fault: plugin-host crash, worker crash, worker timeout,
       registry unavailable, plugin crash mid-schedule, malformed plugin
       result, oversized plugin result, profile with a missing plugin.
    2. For each fault record: detection, behaviour, recovery time, whether
       any schedule/dispatch was issued, duplicated, or lost.
    3. Fix every fault that issues a duplicate command, loses a schedule
       silently, or crashes the control plane; re-drill until clean.
  - **Deliverable / output:** Drill report with per-fault table.
  - **Acceptance criteria / exit check:** Every listed fault has a recorded
    outcome; zero faults crash the control plane or duplicate a command;
    recovery times recorded as measured values.
  - **Owner role:** SDK engineer.
  - **Dependencies:** P7.10, P7.17, P7.18.
  - **Verification method:** Independent witness re-runs two faults and
    reproduces the recorded outcomes.
  - **Risks / failure modes:** Drill run only with polite plugins; a host
    crash treated as acceptable because "the plugin was faulty".

## P7.31 — Handover to Phase 8 (research): research profiles and worker contracts

- [ ] **P7.31 — Handover to Phase 8 (research): research profiles and worker contracts**
  - **Objective:** Hand Phase 8 the research-ready pieces of the SDK —
    the `research-gnn` / `research-marl` profiles, the gRPC worker
    template as the research-worker base, and the proposal-only boundary —
    and list what Phase 8 must still define.
  - **Report trace:** §5.8; §13.4; §23 Stage 8; §22.1 (ADR-010).
  - **Inputs / prerequisites:** P7.15, P7.17; Phase 8 PLAN.md.
  - **Steps:**
    1. List the research profiles as shipped (or deferred) with their
       composition and their no-dispatch-authority property.
    2. List the worker-template features Phase 8 will reuse (Protobuf
       result contract with §9.2 provenance, supervision, isolation) and
       the ones it must extend (GNN/MARL-specific result fields — to be
       defined in Phase 8, not here).
    3. Restate the boundary for Phase 8: research plugins return proposals
       (schedule, candidate setpoint vector, ranking, forecast
       distribution, uncertainty, diagnostics) and never an authorization
       token (§13.4, ADR-010).
    4. Record what is **not** handed over: no GNN/MARL/ADMM provider
       exists yet; no benchmark suite exists yet (Phase 8).
  - **Deliverable / output:** Handover note listing research profiles,
    reusable contracts, the proposal-only boundary, and non-deliverables.
  - **Acceptance criteria / exit check:** Phase 8 can start from the note
    without re-deriving the worker contract; the proposal-only boundary is
    stated; non-deliverables are explicit.
  - **Owner role:** Architect.
  - **Dependencies:** P7.15, P7.17.
  - **Verification method:** Phase 8 lead confirms the note is sufficient
    (or records what is missing).
  - **Risks / failure modes:** Phase 8 assuming a GNN provider contract
    exists because a generic `Optimizer` WIT world exists; the research
    profiles silently inheriting dispatch authority (see P7.15).

## P7.32 — Phase retrospective, open items, and v0.5.0 release-gate record

- [ ] **P7.32 — Phase retrospective, open items, and v0.5.0 release-gate record**
  - **Objective:** Close the phase honestly: record what passed, what did
    not, every open item with an owner, and the v0.5.0 release-gate
    position per §22.3.
  - **Report trace:** §22.3; §23 Stage 7.
  - **Inputs / prerequisites:** P7.01–P7.43; CHECKLIST.md.
  - **Steps:**
    1. Complete CHECKLIST.md: every exit-gate item ticked with evidence,
       or explicitly failed with a reason.
    2. Record the v0.5.0 position against §22.3: which release-gate items
       this phase satisfies, which belong to later phases, and which are
       still open (notably: plugin permission boundaries tested — this
       phase; pilot-readiness items — later).
    3. List deferred work with owners: WASI 0.3 migration (re-evaluate
       after Rust 1.100 stable + host compatibility — see RESEARCH.md),
       UI-extension plugins if deferred in P7.02, any contract deferred
       in P7.02.
    4. State plainly what v0.5.0 is not: not a certification of any plugin,
       not field-control approval; simulation/shadow; no plugin can bypass
       dispatch gates.
  - **Deliverable / output:** Retrospective + open-items register +
    v0.5.0 release-gate record.
  - **Acceptance criteria / exit check:** CHECKLIST.md complete with
    evidence links; every open item has an owner; the v0.5.0 position
    statement exists and contains the simulation/shadow limitation in
    plain language.
  - **Owner role:** Engineering lead + product.
  - **Dependencies:** All P7.01–P7.43.
  - **Verification method:** Reviewer reads CHECKLIST.md and the
    retrospective and can trace every claim to a deliverable from
    P7.01–P7.31.
  - **Risks / failure modes:** A phase declared done with open gate items
    hidden in prose; WASI 0.3 presented as delivered when it was only
    tracked.

---


## P7.33 — Audit-integrity proof: plugin audit records are tamper-evident

- [ ] **P7.33 — Audit-integrity proof: plugin audit records are tamper-evident**
  - **Objective:** Prove the audit records the plugin surface produces
    (P7.11–P7.13, P7.23) cannot be altered, deleted, or dropped by the
    plugin or plugin host they record — audit is a protected-kernel
    invariant (§5.1, §5.7), not a log file a plugin can reach.
  - **Report trace:** §17.3 (audit logging); §17.5; §22.3; §5.1/§5.7.
  - **Inputs / prerequisites:** P7.11 broker; P7.12 matrix; P7.07 lifecycle.
  - **Steps:**
    1. Define the audit-integrity mechanism for plugin audit events
       (append-only store, hash-chained records, or equivalent — record the
       chosen mechanism and why it detects alteration, not merely that
       records are written).
    2. Test tampering: modify, delete, and reorder audit records in a test
       copy and confirm the tampering is detected; attempt the same from
       inside a hostile component and from the plugin-host process.
    3. Test survival: kill the plugin host mid-write and confirm no audit
       event for a completed decision is lost and no partial event is
       presented as complete.
    4. Enforce: a profile switch that would drop audit coverage (e.g.
       removing the audit plugin) is refused (closes the P7.16 recorded
       risk); audit retention is recorded per §16.3.
  - **Deliverable / output:** Audit-integrity design + tamper/survival test
    record.
  - **Acceptance criteria / exit check:** Tampering is detected in every
    test case; host-kill loses no completed-decision event; a switch that
    drops audit coverage is refused; retention is recorded.
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.11, P7.12, P7.16.
  - **Verification method:** Independent re-run of the tamper suite by a
    second engineer on a clean build.
  - **Risks / failure modes:** Integrity checked only on a copy while the
    live store is writable; hash-chaining that nobody ever verifies.

## P7.34 — Plugin-host trust boundary: mTLS, service identity, untrusted host

- [ ] **P7.34 — Plugin-host trust boundary: mTLS, service identity, untrusted host**
  - **Objective:** Define and enforce the trust boundary between the
    plugin host (`apps/plugin-host/`) and the control plane: the host is
    not trusted merely because it is a first-party process — it executes
    untrusted code.
  - **Report trace:** §17.3 (mTLS, short-lived credentials, rotation);
    §18.2 (trust boundary, by analogy); §5.4 Class B; §8.3.
  - **Inputs / prerequisites:** P7.10 host; P7.17 worker template; P9.09
    (pilot certificate practice, as reference only).
  - **Steps:**
    1. Give the plugin host a service identity: mTLS for host ↔ control
       plane, with short-lived certificates, rotation, and revocation;
       record issuance, rotation owner, and expiry monitoring.
    2. Require per-session authentication: the control plane authenticates
       each host session and validates every plugin result it receives
       (schema, §9.2 provenance, tenant scope) before the result enters
       the protected path — a result from the host is untrusted input.
    3. Test a compromised host: a host process that returns forged,
       oversized, or wrong-tenant plugin results — the control plane must
       reject each result and report the host as failed, never act on it.
    4. Test revocation: a revoked host identity is refused a new session
       and its in-flight work is drained, not completed on trust.
  - **Deliverable / output:** Host identity/mTLS design + compromised-host
    test record + rotation/revocation procedure.
  - **Acceptance criteria / exit check:** mTLS with rotation/revocation is
    implemented and tested; every compromised-host result is rejected and
    reported; a revoked identity gets no session.
  - **Owner role:** Security + Rust engineer.
  - **Dependencies:** P7.10.
  - **Verification method:** Run the compromised-host suite; inspect the
    certificate rotation test log.
  - **Risks / failure modes:** mTLS added to the happy path but skipped for
    worker traffic; result validation that trusts the host's own
    self-reported provenance.

## P7.35 — ADR-006 automated enforcement: no-dylib CI gate

- [ ] **P7.35 — ADR-006 automated enforcement: no-dylib CI gate**
  - **Objective:** Enforce ADR-006 mechanically: no arbitrary Rust
    dynamic-library loading exists in the workspace, and CI fails if it
    is introduced — so the CHECKLIST item "No arbitrary Rust dylib loading
    exists" rests on a check that actually runs (it previously cited
    P7.02/P7.28, neither of which runs one).
  - **Report trace:** §22.1 (ADR-006); §5.4; §22.2.
  - **Inputs / prerequisites:** P7.02 scope; P7.27 CI.
  - **Steps:**
    1. Define the ban precisely: no `libloading`/`dlopen`/`dylib` plugin
       loader dependency, no `extern "C"` plugin entrypoint used as a
       loader, no `.so`/`.dll` plugin loading in any form (including via
       a C ABI wrapper as the default mechanism).
    2. Implement the check in CI: dependency grep + code search over the
       workspace (including `plugins/` and examples) that fails the build
       on any hit, with an explicit allowlist mechanism requiring review
       (no silent allowlist entries).
    3. Run the check on the phase branch and record a green run; attempt
       to add a `libloading` dependency on a scratch branch and confirm CI
       fails.
    4. Correct the CHECKLIST.md evidence line for this item to cite this
       subtask.
  - **Deliverable / output:** CI no-dylib check + green run + negative
    scratch-branch demonstration.
  - **Acceptance criteria / exit check:** CI check runs on the SDK code and
    passes; the scratch-branch negative demonstration fails CI; the
    CHECKLIST evidence line cites P7.35.
  - **Owner role:** DevOps + security.
  - **Dependencies:** P7.02, P7.27.
  - **Verification method:** Inspect the CI run; re-run the scratch-branch
    demonstration.
  - **Risks / failure modes:** A check that scans only `crates/` while a
    loader hides in an example; an allowlist that grows without review.

## P7.36 — Replay, sequence, and expiry protection on the plugin path

- [ ] **P7.36 — Replay, sequence, and expiry protection on the plugin path**
  - **Objective:** Prove a plugin-proposed command cannot be replayed,
    reordered, or used after expiry, and that broker duplication on the
    plugin path is handled by the outbox/inbox pattern — not by luck.
  - **Report trace:** §17.3 (signed commands, sequence, expiry, replay
    protection); §17.4 (forged/replayed command, broker duplication/
    replay); §14.3 (sequence, idempotency key, validity window); §15.4–§15.5.
  - **Inputs / prerequisites:** P7.12 matrix; P7.18 supervision; P7.11
    broker.
  - **Steps:**
    1. State the rule for plugin-proposed commands: each carries a
       monotonic sequence/version per asset or control channel, an
       idempotency key, and a valid-from/valid-until window (§14.3); the
       protected path rejects stale, duplicated-with-conflicting-payload,
       out-of-sequence, or expired proposals.
    2. Test replay: capture a valid plugin-proposed command and replay it;
       confirm rejection + audit.
    3. Test expiry and sequence: submit an expired proposal and an
       out-of-sequence proposal; confirm rejection + audit.
    4. Test broker duplication: deliver the same proposal twice through
       the broker (at-least-once delivery, §15.4) and confirm the
       idempotent consumer processes it once (outbox/inbox, §15.5).
  - **Deliverable / output:** Replay/sequence/expiry test record.
  - **Acceptance criteria / exit check:** Every replay, expired, and
    out-of-sequence attempt is rejected and audited; duplicate delivery is
    processed exactly once.
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.11, P7.12, P7.18.
  - **Verification method:** Independent re-run of the replay suite.
  - **Risks / failure modes:** Sequence checked at the broker but not at
    the protected gate, so a replayed proposal that bypasses the broker
    still lands; expiry checked against wall-clock time without a clock
    bound.

## P7.37 — §17.5 safety-invariant test map for the SDK

- [ ] **P7.37 — §17.5 safety-invariant test map for the SDK**
  - **Objective:** Map each of the ten §17.5 safety invariants to the
    named subtask(s) and test(s) that prove it at the plugin boundary, and
    close the one invariant with no test at all.
  - **Report trace:** §17.5 (invariants 1–10); §14.3; §12.5.
  - **Inputs / prerequisites:** P7.11, P7.12, P7.21, P7.23.
  - **Steps:**
    1. Write the map: invariant → enforcing code location → proving test.
       Every invariant must have at least one row; an invariant with no
       test is recorded as open, not as covered by prose.
    2. Close the gap found by this review: invariant 9 — a plugin result
       computed against a missing/uncertain/stale grid model must not pass
       feasibility. Test: a plugin returns a result whose §9.2 provenance
       cites a stale/missing model snapshot; the feasibility gate must
       reject it (not treat the missing model as a pass).
    3. Verify the remaining invariants have live tests, not just design
       statements — in particular invariant 1 (valid asset/site identity
       and current command context) and invariant 6 (no unapproved
       protocol adapter or target) for plugin-proposed commands.
    4. File the map as the SDK's invariant evidence; P7.32 must not close
       the phase while any row is open.
  - **Deliverable / output:** Invariant→test map + invariant-9 test record.
  - **Acceptance criteria / exit check:** All ten invariants mapped with a
    named test; the invariant-9 stale-model test passes (rejection);
    invariants 1 and 6 have live tests cited.
  - **Owner role:** Security engineer + architect.
  - **Dependencies:** P7.11, P7.12, P7.21, P7.23.
  - **Verification method:** Reviewer traces each invariant to its test
    run; any row without a run fails.
  - **Risks / failure modes:** A map that cites design sections instead of
    test runs; invariant 9 "passing" because the feasibility gate was
    never invoked for plugin results.

## P7.38 — Plugin revocation, staged rollout, and failed-plugin removal

- [ ] **P7.38 — Plugin revocation, staged rollout, and failed-plugin removal**
  - **Objective:** Prove a revoked plugin is actually stopped and a new
    plugin version is actually rolled out in stages — and that a plugin in
    the `Failed` state can be drained and removed.
  - **Report trace:** §17.4 (supply-chain compromise: signed artifacts,
    provenance, SBOM, pinned dependencies, staged rollout); §5.6; §9.3.
  - **Inputs / prerequisites:** P7.14 signing; P7.07 lifecycle; P7.15
    profiles.
  - **Steps:**
    1. Test revocation at install: a package whose signature is revoked
       after signing is refused at `Discovered → Verified`.
    2. Test revocation of an installed plugin: revoke the signature of a
       running plugin; the plugin must be stopped/drained and must not
       start again, with the revocation audited.
    3. Test staged rollout: define the rollout stages for a plugin upgrade
       (e.g. one profile first, then wider) and prove an upgrade does not
       go to all profiles at once without a recorded stage decision.
    4. Test removal of a `Failed` plugin, including a malicious one
       (P7.29 scenario): drain, remove, and confirm no residual scheduled
       work from it remains.
  - **Deliverable / output:** Revocation/rollout/removal test record.
  - **Acceptance criteria / exit check:** Revoked-at-install refused;
    revoked-while-running stopped and audited; no un-staged full rollout;
    a `Failed` (including malicious) plugin is removable with no residual
    work.
  - **Owner role:** Security + DevOps.
  - **Dependencies:** P7.14, P7.07.
  - **Verification method:** Independent re-run of the revocation suite.
  - **Risks / failure modes:** Revocation that only blocks new installs
    while the revoked plugin keeps running; a "staged" rollout that is
    really a flag flip.

## P7.39 — Cross-component isolation and data-level tenant filtering

- [ ] **P7.39 — Cross-component isolation and data-level tenant filtering**
  - **Objective:** Prove components in a shared plugin host cannot reach
    each other's state, and that tenant scope is enforced on the data a
    plugin receives — not only on the call that asked for it.
  - **Report trace:** §17.4 (cross-tenant leakage, compromised plugin);
    §5.7; §16.2.
  - **Inputs / prerequisites:** P7.10 host; P7.11 broker; P7.13 sandbox
    tests.
  - **Steps:**
    1. Test cross-component isolation: a hostile component attempts to
       read another component's memory/state, shared host state, or
       another component's outputs through the host; every attempt fails
       and is audited.
    2. Test data-level tenant filtering: a component scoped to site A
       requests telemetry through a legitimate capability call that would
       return site B rows if filtering were call-edge only; confirm the
       returned data contains only site A data.
    3. Test the same two properties for the gRPC worker class (worker
       process shared across tenants, if applicable — record whether
       workers are per-tenant or shared, and test the applicable case).
  - **Deliverable / output:** Cross-component isolation + data-filtering
    test record.
  - **Acceptance criteria / exit check:** Zero cross-component reads
    succeed; zero cross-tenant rows are returned to a scoped plugin in
    either class; the per-tenant vs shared worker model is recorded.
  - **Owner role:** Security engineer.
  - **Dependencies:** P7.10, P7.11, P7.13.
  - **Verification method:** Independent re-run by a second engineer.
  - **Risks / failure modes:** Isolation tested only between polite
    components; tenant filtering that works for telemetry but not for
    forecast/schedule data.

## P7.40 — Toolchain version pinning for WIT/WASM tooling

- [ ] **P7.40 — Toolchain version pinning for WIT/WASM tooling**
  - **Objective:** Pin and record every WIT/WASM toolchain version the SDK
    is built and validated with, so a WIT world that validates under one
    tool version and fails under another is reproducible, not mysterious.
  - **Report trace:** §9.1; §22.2; §10.5.
  - **Inputs / prerequisites:** P7.09 WIT worlds; P7.27 CI; RESEARCH.md §7
    task 2 (only `wasm-tools` 1.239.0 as of 26 August 2026 was verified).
  - **Steps:**
    1. Pin `wit-bindgen`, `cargo-component`, `wasm-tools` (and any WASI
       SDK used) to exact versions in the repo (lockfile/manifest as
       applicable); re-check each against its own release page at
       implementation time and record the check.
    2. Record the pinned versions in the SDK docs and in the P7.28
       exit-gate run book.
    3. Add a CI check that fails if a toolchain version changes without
       the pin record being updated.
  - **Deliverable / output:** Toolchain pin record + CI pin check.
  - **Acceptance criteria / exit check:** Every WIT/WASM tool has an exact
    pinned version with a recorded release-page check; CI fails on an
    unrecorded toolchain change.
  - **Owner role:** DevOps.
  - **Dependencies:** P7.09, P7.27.
  - **Verification method:** Rebuild the WIT packages on a clean machine
    from the pins and confirm identical validation output.
  - **Risks / failure modes:** A "latest" toolchain in CI that silently
    changes WIT validation semantics between runs.

## P7.41 — Manifest-validator fuzz and property tests

- [ ] **P7.41 — Manifest-validator fuzz and property tests**
  - **Objective:** Test the manifest validator beyond its fixed example
    set, with fuzzing and property tests — it stands between an untrusted
    plugin author and activation.
  - **Report trace:** §19.2(2) (property tests); §5.5; §5.7.
  - **Inputs / prerequisites:** P7.05 schema + validator.
  - **Steps:**
    1. Write property tests for the validator: generated manifests with
       random field combinations never validate when a required field is
       missing, a SemVer range is malformed, a runtime kind is unknown,
       or an undeclared permission is requested.
    2. Fuzz the validator with malformed manifests (truncated JSON/YAML,
       oversized fields, duplicate keys, deep nesting, non-UTF8 input)
       for a recorded duration; every input must be accepted, rejected,
       or error cleanly — never crash or hang the validator.
    3. Property-test SemVer range parsing specifically: ranges are parsed
       as ranges, never string-compared (P7.05), across generated range
       expressions.
  - **Deliverable / output:** Validator fuzz/property test suite + run
    record.
  - **Acceptance criteria / exit check:** Property tests pass; the fuzz run
    completes with zero crashes/hangs and every crash-class input
    dispositioned; SemVer parsing is range-based on generated inputs.
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.05.
  - **Verification method:** Re-run the fuzz suite from the recorded seed/
    duration and compare the outcome class counts.
  - **Risks / failure modes:** Fuzzing only the happy-path grammar; a
    validator that rejects malformed input in the lab but panics on it in
    the host.

## P7.42 — Plugin-path determinism and seeded replay

- [ ] **P7.42 — Plugin-path determinism and seeded replay**
  - **Objective:** Apply §24 step 10 replay reproducibility to the plugin
    path: the same scenario through the plugin host with the same versions
    and seeds reproduces the same proposed schedule within a stated
    tolerance.
  - **Report trace:** §24 step 10; §19.2(5) (deterministic scenario
    tests); §9.2.
  - **Inputs / prerequisites:** P7.10 host; P7.17 worker; P7.22 swap test;
    P7.28 run book.
  - **Steps:**
    1. Define the replay: the §24 slice scenario run through the plugin
       host (one WASM component, one gRPC worker) with pinned versions
       (FlexCore, Wasmtime per P7.40, WIT worlds, worker template, plugin
       versions) and seeds recorded.
    2. Run the scenario twice from the same inputs; compare the proposed
       schedules (objective, feasibility verdict, §9.2 provenance) within
       a tolerance recorded *before* the second run.
    3. Record the nondeterminism sources found (fuel-limit behaviour,
       Wasmtime-version sensitivity, worker timing) and state which are
       excluded from the equivalence claim and why.
    4. File the replay record with the P7.28 evidence pack.
  - **Deliverable / output:** Plugin-path replay record (inputs, both runs,
    comparison, tolerance, nondeterminism notes).
  - **Acceptance criteria / exit check:** Both runs reproduce within the
    pre-recorded tolerance; nondeterminism sources are listed with their
    treatment; the record is filed with the exit-gate evidence.
  - **Owner role:** Rust engineer (SDK).
  - **Dependencies:** P7.10, P7.17, P7.22.
  - **Verification method:** A second engineer replays from the record
    alone and confirms reproduction within tolerance.
  - **Risks / failure modes:** Timing compared as if it were a result;
    a tolerance fitted after seeing the second run's difference.

## P7.43 — Scenario-regression suite for the SDK

- [ ] **P7.43 — Scenario-regression suite for the SDK**
  - **Objective:** Define the §22.2 scenario-regression suite for the SDK
    — named scenarios with pinned inputs, seeds, and expected outcomes —
    so the CI job P7.27 wires up has contents, and an empty suite cannot
    be "green".
  - **Report trace:** §22.2 (scenario regression suite); §19.2(5); §24.
  - **Inputs / prerequisites:** P7.22 swap test; P7.21 conformance suite;
    P7.27 CI.
  - **Steps:**
    1. Name the scenarios: the §24 slice scenario (baseline + both swaps
       from P7.22), the lifecycle scenario (install → configure →
       activate → upgrade → drain → remove, P7.07), the denial scenario
       (P7.23 attempts), and the limit scenario (P7.13 limit behaviour).
       Each scenario records its inputs, seeds, versions, expected
       outcome, and tolerance.
    2. Version the suite as an artifact; every CI run records the suite
       version.
    3. Wire the suite into CI (P7.27) on every pull request touching SDK
       code; a scenario failure blocks the merge.
    4. Record the first green run with the suite version.
  - **Deliverable / output:** Scenario-regression suite definition +
    versioned artifact + CI wiring + green run.
  - **Acceptance criteria / exit check:** Every named scenario has pinned
    inputs/seeds/expected outcome/tolerance; the suite runs in CI on SDK
    pull requests; a failing scenario blocks merge; the suite version is
    recorded per run.
  - **Owner role:** DevOps + SDK engineer.
  - **Dependencies:** P7.22, P7.21, P7.27.
  - **Verification method:** Break one scenario's expected outcome on a
    scratch branch and confirm CI fails.
  - **Risks / failure modes:** A suite that grows silently between runs,
    making comparisons incomparable; scenarios whose expected outcomes
    were fitted to observed results.

## Subtask index

| ID | Title | Trace |
|---|---|---|
| P7.01 | Phase readiness check (Phases 1–6, P6.29 handover) | §23 Stages 1–6 |
| P7.02 | SDK scope freeze (contracts, categories, kernel) | §5.1–§5.3, §9.1 |
| P7.03 | License/dependency audit (SDK stack) | §25.2, §9.3 |
| P7.04 | Capability-seam inventory (contract → provider → consumer) | §5.2, §9.1 |
| P7.05 | Plugin manifest schema (versioned, validated, published) | §5.5, §9.3 |
| P7.06 | Dependency resolution (missing/incompatible/cyclic/conflicting) | §5.5–§5.6 |
| P7.07 | Plugin lifecycle state machine (drain-before-replace) | §5.6 |
| P7.08 | Built-in Rust provider registry (Class A) | §5.4 Class A |
| P7.09 | WIT interface packages (versioned worlds) | §5.4 Class B, §9.1 |
| P7.10 | Wasmtime component host prototype (separate process) | §5.4 Class B, §8.3 |
| P7.11 | Host capability broker (scoped capabilities) | §5.4 Class B, §5.7 |
| P7.12 | Permission enforcement (manifest permissions enforced) | §5.7, §9.1 |
| P7.13 | Sandbox negative tests (escape, exhaustion, ambient access) | §5.4 Class B, §17.4 |
| P7.14 | Plugin signing, provenance, SBOM | §9.3, §5.6 |
| P7.15 | Composition profiles and bundles | §5.8 |
| P7.16 | Profile switching and safe re-composition | §5.6, §5.8, §21.4 |
| P7.17 | Python gRPC worker template (Class C) | §5.4 Class C, §7.2 |
| P7.18 | Worker supervision (timeouts, retries, isolation, backpressure) | §5.4 Class C, §19.2 |
| P7.19 | Plugin configuration (schema validation, secret references) | §5.5–§5.7, §17.3 |
| P7.20 | Plugin health, readiness, observability | §5.5–§5.6, §21.3 |
| P7.21 | Plugin conformance suite (all classes, same suite) | §19.2, §9.3 |
| P7.22 | Swap test (optimizer + grid provider, no app/domain change) | §23 Stage 7 |
| P7.23 | Untrusted-plugin negative test (ungranted capabilities) | §23 Stage 7, §5.7 |
| P7.24 | SDK documentation and examples (incl. deliberately bad example) | §8.1 |
| P7.25 | Operator console: Plugins & Integrations workspace | §20.1–§20.2 |
| P7.26 | Performance/resource validation (host and workers) | §19.2, §19.4 |
| P7.27 | CI gates (fmt/clippy/tests, Buf, WIT, manifest, SBOM) | §22.2, §10.5 |
| P7.28 | Exit-gate run (swap + untrusted-plugin proofs, witnessed) | §23 Stage 7 |
| P7.29 | Security testing (supply chain, escalation, host compromise) | §19.2, §17.4 |
| P7.30 | Failure-mode drill (host, workers, registry) | §19.2, §21.2 |
| P7.31 | Handover to Phase 8 (research profiles, worker contracts) | §5.8, §13.4 |
| P7.32 | Retrospective, open items, v0.5.0 release-gate record | §22.3 |
| P7.33 | Audit-integrity proof (plugin audit records tamper-evident) | §17.3, §17.5 |
| P7.34 | Plugin-host trust boundary (mTLS, untrusted host) | §17.3, §18.2 |
| P7.35 | ADR-006 automated enforcement (no-dylib CI gate) | §22.1, §5.4 |
| P7.36 | Replay, sequence, expiry protection (plugin path) | §17.3–§17.4, §14.3 |
| P7.37 | §17.5 safety-invariant test map (SDK) | §17.5 |
| P7.38 | Plugin revocation, staged rollout, failed-plugin removal | §17.4, §5.6 |
| P7.39 | Cross-component isolation, data-level tenant filtering | §17.4, §16.2 |
| P7.40 | Toolchain version pinning (WIT/WASM tooling) | §9.1, §22.2 |
| P7.41 | Manifest-validator fuzz and property tests | §19.2, §5.5 |
| P7.42 | Plugin-path determinism and seeded replay | §24, §19.2 |
| P7.43 | Scenario-regression suite (SDK) | §22.2, §19.2 |

**Subtask count: 43.**
