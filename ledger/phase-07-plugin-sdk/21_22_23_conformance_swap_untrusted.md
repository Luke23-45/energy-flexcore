# P7.21/P7.22/P7.23 — Conformance suite, swap test, untrusted-plugin negatives

Date: 2026-11-13. Subtasks: P7.21, P7.22, P7.23 (`ledger_phase_7.md` §P7.21,
§P7.22, §P7.23). Owner role: Rust test engineer (SDK). Report trace: §19.2(3),
§9.3, §22.2, §5.2, §5.6, §5.7, §5.8, §12.3–§12.4, §13.4, §17.4, §23 Stage 7.
Inputs: P7.04 (`04_seam_inventory.md` — the seed-provider inventory and the
solver-class declarations); P7.05–P7.08 (`05_06_manifest_resolution.md`,
`07_08_lifecycle_registry.md`); P7.10–P7.13 (`10_11_plugin_host_broker.md`,
`12_13_enforcement_negatives.md`); P7.14–P7.16
(`14_15_16_signing_profiles_recomposition.md`); P7.19/P7.36
(`19_20_36_config_health_path.md`); the P6.19 conformance precedent
(`crates/edge-agent/tests/conformance_suite_p6_19.rs`) and the P6.13 harness
idea (`crates/edge-agent/src/conformance.rs`); the real providers and their
consumers (`crates/flexcore-application/src/`, `crates/flexcore-ports/src/`,
`crates/flexcore-optimization-api/src/`, `crates/flexcore-grid-api/src/`,
`apps/sim-runner/src/`); `plugins/manifests/*.json`, `plugins/profiles/*.json`,
`python/workers/contracts/worker_api.proto` (field names, read-only).

## 0. Honest status — READ FIRST

**Everything in this wave is WRITTEN-UNRUN.** No cargo command was executed
(no install, no build, no test): every test in
`crates/flexcore-kernel/tests/{conformance_p7_21,swap_p7_22}.rs` and
`apps/plugin-host/tests/untrusted_plugin_p7_23.rs` is WRITTEN-UNRUN, and the
first human build is the verification gate. No claim in this record is a
tool-verified fact; statements about behaviour are design statements over the
code as written. The ledger tick lines read "implemented, awaiting human
build/test" — never "tested"/"verified"/"passed".

Where a claim CANNOT be made from the current seams at all, it is recorded as
a **HUMAN GATE** item (§5) or a **SEAM GAP** with the minimal exposure needed
(§3.3). No claim of external certification of any kind is made anywhere.

## 1. What was built (per-subtask coverage)

| Subtask | Delivered | Where |
|---|---|---|
| P7.21 class-agnostic contract suite `flexcore-plugin-sdk-conformance-suite-v1` | 36 class-agnostic cases over the kernel seams: manifest validity (schema + named errors), api-version compatibility vs `CURRENT_PLATFORM_API_VERSION`, permission grammar (never-granted refused, ceiling containment), secret-REFERENCE-only config, closed resource bounds, determinism metadata (request id/sequence/payload-bound digest — the kernel-side projection of the P7.17/P7.42 seed/request_id contract), provenance-record conformance (P7.14), the §5.6 legal transition set, audit-trail shape, and the protected-kernel guard (resolver + plugin-path restatement) | `crates/flexcore-kernel/tests/conformance_p7_21.rs` |
| P7.21 per-class case groups | Class A: registry registration from a plan (protected/duplicate refusals). Class B: world conformance against the recorded `wit/` worlds (mirror, read-only); the bindings seam honestly UNRUN. Class C: `worker_api.proto` presence rules (`contract_version`, `DeterminismMetadata { seed, request_id, seed_supported }`) mirrored read-only, plus the `target` entry-point rule | same file (cases #37–#43) |
| P7.21 per-class run records + suite version | The SAME five-case manifest-layer set runs against one fixture per class (A/B/C) and records `class=… suite=… version=1 cases=n/5` — no class-specific shortcut (the ledger's named risk) | same file (`per_class_run_record`) |
| P7.21 CI wiring | SEAM RECORDED, nothing implemented: the P7.27 wave owns the CI job (config-only); see §2.4 | §2.4 of this record |
| P7.22 swap test | The seam property: optimizer swap (HiGHS-class → Clarabel-class) and a second grid-provider swap (pandapower-class → OpenDSS-class stand-in) through `swap_provider` + drain-before-replace ONLY; blocked-then-allowed asserted; (a) same port trait, (b) ADR-009 independent feasibility validation preserved (test-local mirror + recorded seam gap), (c) solver-class discipline intact incl. the MILP/Clarabel class-mismatch impossibility, (d) audit ordering refusal → drain → VERSION_CHANGE | `crates/flexcore-kernel/tests/swap_p7_22.rs` |
| P7.23 untrusted-plugin negatives | The end-to-end layered proof (L1 ceiling → L2 door → L3 broker grant → L4 broker check → L5 binding seam) with the refusal at the EARLIEST layer and bypass-then-next-layer-refuses per attack; the ADR-010 attempt through every layer; the class-lie and trusted-bypass cases; grant forgery at composition level; cross-tenant reads; every denial audited | `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` |

## 2. P7.21 — the plugin conformance suite

### 2.1 Suite identity and claim boundary

- Suite id: `flexcore-plugin-sdk-conformance-suite-v1`; suite version: `1`
  (recorded with every run — the per-class record line carries both).
- The suite is 100% FlexCore-side: it tests the plugin-SDK contract surface
  as implemented in the kernel (manifest, resolver, profile, signing,
  lifecycle, plugin config, plugin path, registry). No external protocol or
  certification program is involved.
- **CLAIM BOUNDARY (verbatim, pinned by test
  `the_suite_identity_and_claim_boundary_are_pinned`):**

  > Passing this suite entitles FlexCore to claim exactly: "passes the
  > FlexCore plugin SDK conformance suite version 1". It certifies NOTHING
  > externally: no protocol, security, or safety certification of any kind,
  > and no permission to control field devices (the entire 0.x line is
  > simulation-only). Where a layer needs a real WASM component or gRPC
  > worker, the suite asserts the structural guarantee that exists and the
  > residual is a recorded human-gate item.

- The full case→requirement table (44 rows, every case mapped to
  §19.2(3)/§5.x/§9.x or marked FLEXCORE-ONLY) is the header table of
  `conformance_p7_21.rs` — the P6.19 pattern. It is not duplicated here;
  the highlights: negative cases are first-class (the ledger's named risk
  "a suite that only tests the happy path" is countered: 20+ of the cases
  are refusals), and no class is exempted from a case another class must
  pass (the per-class record runs the SAME case set per class).

### 2.2 What the class-agnostic suite exercises (and what it honestly cannot)

Exercised against kernel seams (no class can skip any of it): the P7.05
manifest validator (closed key sets, named errors), the P7.06 resolver
(platform-api gate, §9.3 range intersection, protected-kernel guard), the
P7.15 profile ceiling containment, the P7.05/P7.19 secret-REFERENCE-only
rules (the value is never echoed anywhere), the closed resource ranges, the
P7.36 plugin-path identity (correlation id/request id, sequence ≥ 1,
deterministic payload-bound digest, expiry refusal shape, ADR-010 proposal
envelope), the P7.14 provenance gate (unsigned refused before anything else;
SBOM reference required; signer key is a scheme-prefixed reference), the §5.6
frozen transition table (full 10×10 enumeration = 12 frozen + 5 crash edges),
and the audit-record shape (§5.6 step 4 fields + injected-clock timestamps +
deterministic emission order).

Honest limitation: the suite drives DATA and STRUCTURE seams. It does not
execute plugin code (nothing in the kernel can), does not run a real WASM
component, and does not call a real gRPC worker. Those runtime layers are
the human-gate items in §5 (same split discipline as P7.13 §7).

### 2.3 Class-specific groups

- **Class A** — registers through the registry from a resolved plan
  (`register_from_plan` full coverage in both directions), constructs on
  demand, answers through the port trait, refuses protected-kernel and
  duplicate factories. The SWAP itself is P7.22's file (cross-referenced,
  not duplicated).
- **Class B** — the manifest's `entry-point.symbol` must name the recorded
  `wit/` world namespace (`flexcore:`; the three recorded worlds are
  mirrored READ-ONLY from `apps/plugin-host/src/host.rs::WorldId` and the
  `wit/` tree — the kernel cannot import the host crate). DEFAULT decision:
  the manifest schema v1 carries no dedicated world field, so the
  package-namespace check is the structural rule the suite enforces
  kernel-side; the exact package-id↔world binding materialises at the
  bindings build (human gate). The bindings seam itself is asserted
  honestly UNRUN: the kernel exposes NO execution surface at all (case
  `class_b_bindings_seam_is_honestly_unrun`).
- **Class C** — the `worker_api.proto` field names are mirrored READ-ONLY
  (`contract_version`, `request_id`, `seed`, `seed_supported`); the
  presence rules are pinned: `contract_version` mandatory and
  major-skew-refused (`UNSUPPORTED_CONTRACT_VERSION` semantics),
  `request_id` non-empty, `seed` present, and an honest
  `seed_supported = false` admission well-formed (excluded from the P7.42
  equivalence claim, recorded — not penalised).

### 2.4 CI wiring — RECORDED SEAM (P7.27 owns it)

P7.21 step 3 ("add the suite to CI") is the P7.27 wave's deliverable:
**nothing was added to `ci.yml`** (out of this wave's ownership; the shared
file rule). Recorded seam: the P7.27 CI job should run
`cargo test -p flexcore-kernel --test conformance_p7_21` and
`cargo test -p flexcore-plugin-host --test untrusted_plugin_p7_23` (plus the
existing suites), and the P7.21 ledger acceptance "a plugin package cannot be
published/activated without a passing suite run recorded against its
version" is a publishing-pipeline rule for that wave to wire, not a code
change here. The suite version (1) is recorded in the suite constants so the
CI record can quote it.

## 3. P7.22 — the swap test

### 3.1 The seam property, as proven (written-unrun)

`crates/flexcore-kernel/tests/swap_p7_22.rs` composes the P7.08 registry and
the P7.07 lifecycle and proves:

1. **Zero application/domain edits**: the consumer functions
   (`scheduling_use_case`, `independent_feasibility_validation`) are defined
   ONCE before any swap, take ONLY the port trait (`&dyn Solver`,
   `&dyn GridPort`), and are called unchanged before and after both swaps.
   The swap is performed exclusively through
   `ProviderRegistry::swap_provider` + `begin_drain`/`complete_drain` —
   there is no other replacement API, so "no direct registry mutation while
   Active" holds by construction.
2. **Blocked-then-allowed**: a swap while the incumbent is Active (Ready) is
   `LIFECYCLE_REFUSED` wrapping `REPLACEMENT_BLOCKED_UNDRAINED`; a swap
   mid-drain (Draining, not quiesced) is refused identically; the factory is
   unchanged after each refusal; only a fully drained (Stopped) incumbent is
   replaced. Refusals are audited.
3. **(a) Same port trait**: after the swap, `registry.construct` answers
   through the same `Arc<dyn Solver>`/`Arc<dyn GridPort>` object type; the
   contract's plugin identity is stable; the new provider's name is the
   swapped-in implementation.
4. **(b) ADR-009 preserved** (the baseline requirement): the independent
   feasibility validation consumes the SWAPPED optimizer's output and
   reaches its own verdict through the GRID port — the optimizer's own word
   is never the verdict, before AND after the swap. In this file that is the
   test-local mirror `independent_feasibility_validation`; the REAL
   application seam is a recorded seam gap (§3.3, gap 2).
5. **(c) Solver-class discipline**: the §7.4 declarations ride the port
   trait (`solver_class`): HiGHS-class = "LP/QP/MIP … not SOCP", Clarabel-class =
   "convex conic … not MISOCP, no integer variables"; the swap is exercised
   only on the LP formulation, and the class-mismatch failure mode the
   ledger names is asserted IMPOSSIBLE (the Clarabel-class provider does not
   support the MILP problem, so a Clarabel swap on a MILP formulation yields
   no schedule — the formulation change, had one been needed, would be a
   problem-definition change, not a code change).
6. **(d) Audit ordering**: refused (blocked swap) < Ready→Draining <
   Draining→Stopped < VERSION_CHANGE in emission order; the version record
   carries the OLD version with the NEW version in the reason; the grid swap
   follows the same path in its own lifecycle.
7. **Pre-declared equivalence criteria** (ledger step 2, declared in module
   constants BEFORE either swap): same feasibility verdict for the LP
   fixture; same objective within tolerance 0 (basis: integral-data fixture
   LP with a unique exact optimum); same §9.2 completeness (provenance AND
   diagnostics non-empty). Timing is a measurement at the witnessed run,
   never a criterion.
8. **Refusal taxonomy**: `SWAP_CONTRACT_UNKNOWN`, `SWAP_LIFECYCLE_MISMATCH`,
   `PROTECTED_KERNEL_CONTRACT_REFUSED` (checked FIRST), and the unchanged
   state after every refusal.

The grid-provider swap records the D3 fidelity point: the declared analysis
class ("balanced AC power-flow evaluation — NOT an AC-OPF optimization") is
asserted UNCHANGED across the swap; a fidelity change would be a recorded
problem-definition change (D3), not a silent property of a swap.

### 3.2 Provider honesty

The kernel crate depends on no `flexcore-*` crate (the P7.05/P7.06
dependency decision), so the REAL `HighsTier1Provider`,
`ClarabelTier2Provider`, and `PandapowerGridProvider` cannot be constructed
inside the kernel test crate. Per the recorded P7.08 in-test rule, the swap
test uses LOCAL doubles over trivial local port traits, named after the real
providers via the registry's seed constants and carrying the seam inventory
§4 solver-class declarations verbatim. The REAL registration/swap is a
composition-root line through the same `FnFactory` adapter; the real-provider
witnessed run is the P7.28 exit-gate human gate.

### 3.3 SEAM GAPS found (named, with the minimal exposure needed — nothing edited)

| # | Gap | Why it exists | Minimal exposure needed (NOT performed — would require edits outside this wave's ownership) |
|---|---|---|---|
| 1 | The real providers cannot participate in the in-kernel swap test | `crates/flexcore-kernel` depends on no `flexcore-*` crate by design; its test crate can therefore not construct `HighsTier1Provider`/`ClarabelTier2Provider`/`PandapowerGridProvider`, and adding dev-dependencies would require a `Cargo.toml` edit (banned this wave) | None for the seam property (the doubles carry the real names and class declarations); for the real-provider witnessed run, the P7.28 composition root constructs the real providers through their PUBLIC constructors (`HighsTier1Provider::new(Arc<dyn Tier1Backend>)`, `ClarabelTier2Provider::new(Tier2ProviderConfig)`, `PandapowerGridProvider::new(backend, scope)`) via `FnFactory` — no source edits anywhere |
| 2 | The ADR-009 independent feasibility validation's REAL seam is not callable from any existing test crate | The real seam is `flexcore_application::command_gates::{run_validation_gate, run_feasibility_gate}` (the `CommandFeasibilityRunner` trait over the `GridModelProvider` chain, `crates/flexcore-application/src/command_gates.rs` lines 507/1135/1323). No crate in the workspace depends on BOTH `flexcore-kernel` (the registry under test) AND `flexcore-application`/`flexcore-grid-api`/`flexcore-optimization-api` (the seam + providers): `apps/sim-runner` has the latter but not the kernel; the kernel has neither. Composing them requires a dependency edit | Add two path dependencies to `apps/sim-runner/Cargo.toml` (a composition root; both edges are allowed composition-root edges per `tools/check_dependency_rule.py`): `flexcore-kernel` and `flexcore-optimization-api`. Then `apps/sim-runner/tests/swap_p7_22_integration.rs` can register the REAL providers from a real plan, swap through `swap_provider`, and drive `run_validation_gate`/`run_feasibility_gate` over the swapped output — the P7.28 witnessed run's exact shape. The test file was deliberately NOT created (`apps/sim-runner/tests/` is listed in the ownership as optional ONLY if no edits were needed; they are) |
| 3 | The §23 Stage 7 full scenario re-run (§24 slice, recorded timings, `git diff` over application/domain code) is not executable here | It needs the composition of gap 2 plus a runnable scenario; the loop bans forbid building/running | The P7.28 exit-gate witnessed run (its ledger entry already requires P7.22; CHECKLIST.md carries the witness procedure) |

## 4. P7.23 — the untrusted-plugin negative test

### 4.1 The layer model (defence in depth, refusal at the earliest layer)

| Layer | Enforcing code | Refusal (named) | Audited where |
|---|---|---|---|
| L0 kernel manifest gate | `crates/flexcore-kernel/src/manifest.rs` (CITED — not callable from this crate; §8.2 dependency rule) | `PERMISSION_NEVER_GRANTED` — a parsed manifest structurally cannot carry a never-granted id | kernel-side (out of scope here) |
| L1 profile ceiling | `enforcement.rs::evaluate` step 3 (ceiling soundness) + step 5 (intersection) | `PERMISSION_NEVER_GRANTABLE` (ceiling entry), `PERMISSION_ABOVE_PROFILE` (declared above ceiling) | `PLUGIN_INSTANTIATION_REFUSED` |
| L2 enforcement door | `enforcement.rs::evaluate` step 4 | `PERMISSION_NEVER_GRANTABLE`, `PERMISSION_UNKNOWN`, `PERMISSION_DUPLICATE_DECLARATION` | `PLUGIN_INSTANTIATION_REFUSED` |
| L3 broker grant | `broker.rs::grant` / `derive_grants` | `PERMISSION_NEVER_GRANTABLE`, `PERMISSION_UNKNOWN`, `PERMISSION_NOT_DECLARED`, `SCOPE_INVALID` | HONEST NOTE: a direct broker grant-refusal RETURNS the named error without a broker audit record (the audit-or-refuse chain covers USES and the enforcement door, which audits every refusal that flows through it); the behavioural refusals at L3 in this suite are all reached through the door in real composition |
| L4 broker check | `broker.rs::check` (per-call, defaults deny, no cached authority) | `PERMISSION_NEVER_GRANTABLE`, `PERMISSION_UNKNOWN`, `CAPABILITY_NOT_GRANTED`, `SCOPE_OUTSIDE_GRANT`, `GRANT_EXPIRED` | `PLUGIN_CAPABILITY_DENIED` (every denial) |
| L5 binding seam | `enforcement::instantiate` → `bindings::NoBindings` (default build) | `BINDINGS_NOT_GENERATED` on every use | loader refusals named (`COMPONENT_NOT_FOUND`); the gated-build teardown audit is the human gate |

For EVERY never-granted capability (`dispatch.authorize`, `dispatch.issue`,
`secret.read`, `network.any`) the layered walk asserts: L1 refuses first;
bypassing L1 (clean ceiling, lying manifest) → L2 refuses; bypassing L1+L2
(direct broker call) → L3 refuses; bypassing L3 (crafted check call) → L4
refuses AND audits. The earliest-layer rule and the next-layer-still-refuses
rule are both asserted per attack (the ledger's "defense in depth" wording,
made concrete).

### 4.2 The attack table (attempt → earliest refusal → audit)

| Attack (ledger P7.23 step 1) | Earliest host refusal | Audit |
|---|---|---|
| Read another tenant's telemetry | L4 `SCOPE_OUTSIDE_GRANT` (tenant-b, other site, tenant-wide ask — 3 attempts, each audited; grants are per-plugin, never shareable: `CAPABILITY_NOT_GRANTED` for a second plugin) | 3 × `PLUGIN_CAPABILITY_DENIED` |
| Propose a dispatch as if authorized (ADR-010) | The proposal is DATA: the validation forward returns `()` — no verdict type exists in any reachable signature; authorize/issue stay `PERMISSION_NEVER_GRANTABLE` at L4 even with proposal grants in hand; a validate-request grant aimed anywhere but `protected-validator` is `SCOPE_INVALID` at L3 | `PLUGIN_VALIDATION_FORWARD_REQUESTED` + denials |
| Call the authorizer (`dispatch.authorize`) | L1 (ceiling cannot contain it) → bypass chain L2 → L3 → L4 all refuse | door refusals + `PLUGIN_CAPABILITY_DENIED` |
| Issue a command (`dispatch.issue`) | same chain | same |
| Read a secret (`secret.read`) | L1→L4 chain; additionally no env/storage capability id exists (`PERMISSION_UNKNOWN` for crafted ids) | denials |
| Open the network (`network.any`) | L1→L4 chain; socket-shaped ids `PERMISSION_UNKNOWN` | denials |
| Write another module's tables | `PERMISSION_UNKNOWN` at L3 and L4 — no storage/database capability id exists (ADR-007 protected non-seams; a plugin cannot even ADDRESS the repositories) | denials |
| Manifest lies about its class | FLEXCORE-ONLY: `EnforcementRequest` carries NO class field — nothing to lie with; a first-party-looking plugin id is still subject to the ceiling (`PERMISSION_ABOVE_PROFILE` against the empty DEFAULT ceiling) and authorityless until a derivation ran for ITS id (`CAPABILITY_NOT_GRANTED`) — the P7.12 "trusted class bypasses enforcement" failure mode closed at both sides | door refusals |
| Claim a protected-kernel contract (`kernel.command-authorizer`, …) | End-to-end restatement: the composition-level refusal is the kernel resolver/registry's (`PROTECTED_KERNEL_REPLACE` — cited; P7.06/P7.08); host-side, the vocabulary has no `kernel.*` id at all (`PERMISSION_UNKNOWN`), and the sweep asserts NO grantable id names the kernel or a verdict | denials |
| Grant forgery at composition level | STRUCTURAL: `CapabilityGrant` fields are `pub(crate)` and the broker has no import API (a struct literal is a compile error). BEHAVIOURAL: a captured receipt is inert in a fresh broker (`CAPABILITY_NOT_GRANTED`), dead after revocation, expired per call (`GRANT_EXPIRED`) | revocation records |
| Research-profile context (research-gnn / research-marl) | `PERMISSION_ABOVE_PROFILE` at the door for `schedule.propose` and `dispatch.propose` — the P7.15 no-dispatch ceiling bound to this run | door refusals |
| Everything allowed, real execution attempted | L5: the verdict's binding is the `NoBindings` object — every use `BINDINGS_NOT_GENERATED`; the loader refuses absent artifacts (`COMPONENT_NOT_FOUND`) | human-gate item for the gated build |

### 4.3 Human-gate items (the residual, same split as P7.13 §7)

1. A real (gated-build) WASM component attempting the same attacks from
   inside a live instance — teardown/limit behaviour is the P7.13 §7 list;
   this suite proves the host API refuses before any code runs.
2. The same attempts from a real gRPC worker context (Class C) — the
   P7.17/P7.20 seam (recorded, not built).
3. The wasmtime CVE-floor advisory re-check (`12_13_enforcement_negatives.md`
   §9) — unchanged, still open at its owner.

## 5. WRITTEN-UNRUN vs human-gate split (summary table)

| Proof | Status here | Where the run happens |
|---|---|---|
| Conformance suite cases (kernel seams) | WRITTEN-UNRUN | first human `cargo test -p flexcore-kernel --test conformance_p7_21` |
| Untrusted-plugin layered negatives (host API) | WRITTEN-UNRUN | first human `cargo test -p flexcore-plugin-host --test untrusted_plugin_p7_23` |
| Swap seam property (doubles) | WRITTEN-UNRUN | same first build |
| Swap with REAL providers + real independent feasibility gate + §24 scenario + timings + `git diff` | NOT RUN — seam gap 2 | P7.28 exit-gate witnessed run (after the sim-runner dependency exposure) |
| Live WASM/gRPC hostile components | NOT RUN — no artifacts exist | P7.13 §7 human-gate list / P7.24 examples / P7.28 witness |
| CI wiring of the suite | NOT DONE — P7.27 owns it | P7.27 CI job |

## 6. DEFAULT decisions (one list)

1. **Suite world-classification rule (P7.21, Class B):** the manifest
   schema v1 carries no world field; the suite classifies by the
   `entry-point.symbol` package namespace (`flexcore:`) and treats a
   foreign namespace as a suite failure. The exact package-id↔world binding
   materialises at the bindings build (human gate).
2. **Determinism-metadata projection (P7.21):** the kernel-side projection
   of the P7.17/P7.42 `seed`/`request_id` contract is the P7.36 plugin-path
   identity (non-empty correlation id, sequence ≥ 1, deterministic
   payload-bound digest, expiry refusal); the literal proto field
   presence rules are pinned in the Class C group. The proto file itself is
   read-only mirrored, never re-declared.
3. **In-test provider doubles (P7.22):** local port traits + doubles named
   via the registry seed constants (the P7.08 record's established rule),
   with the seam inventory §4 class declarations verbatim; the witnessed
   real-provider run is P7.28's.
4. **Equivalence tolerance (P7.22 step 2):** objective tolerance 0 on the
   fixture LP (basis: integral data, unique exact optimum); timing is a
   measurement, never a criterion.
5. **Fidelity handling in the grid swap:** the declared analysis class is
   asserted unchanged; any fidelity change is a D3 problem-definition
   record, never a silent swap property.
6. **Layer walk shape (P7.23):** "bypass" is simulated at the API level
   (clean ceiling / direct broker calls / crafted check calls) — there is
   no other bypass to simulate, because no code runs anywhere in this
   build (L5 is the structural backstop).
7. **L3 audit honesty:** a direct broker grant-refusal is returned without
   a broker audit record (the broker audits grant ISSUE, revoke, USE, and
   check-denials; the enforcement door audits every refusal that flows
   through it). Recorded here so the exit-gate record does not over-claim
   audit coverage at L3.
8. **P7.23 artifact fixture:** the binding-seam case writes a throwaway
   temp file (bytes irrelevant to the default seam) so the L5 refusal is
   exercised end-to-end; the file is removed after the case.

## 7. File inventory (ownership)

NEW (this wave):
`crates/flexcore-kernel/tests/conformance_p7_21.rs`,
`crates/flexcore-kernel/tests/swap_p7_22.rs`,
`apps/plugin-host/tests/untrusted_plugin_p7_23.rs`, this record.

EDITED (ticks only):
`ledger/phase-07-plugin-sdk/ledger_phase_7.md` — P7.21/P7.22/P7.23 tick
lines ONLY (P7.19/P7.20/P7.36 belong to the concurrent kernel/host wave;
their lines were not touched).

NOT TOUCHED: all crate `src/**` (both kernels' waves' crates were treated as
READ-ONLY per the task; no `pub` exposure was needed beyond what exists —
seam gap 2 records the ONE exposure that would be needed, on the sim-runner
manifest, which was not edited), `docs/foundations/*`, `docs/plan/*`,
`.agent/*`, `wit/*`, `ci.yml`, root `Cargo.toml`,
`tools/check_dependency_rule.py`, `python/workers/contracts/worker_api.proto`
(read-only mirror).

## 8. Acceptance check (ledger P7.21/P7.22/P7.23 — honest)

- P7.21 "the same interface suite for every adapter and plugin": the
  class-agnostic case set runs per class and the per-class record is
  asserted clean for A, B, and C — WRITTEN-UNRUN (design-level pass over
  the fixtures as written).
- P7.21 "a plugin returning a result without §9.2 provenance/diagnostics
  fails the suite": pinned twice — the P7.14 provenance gate refuses
  unsigned/incomplete records before activation, and the P7.22 mirror's
  independent validation refuses incomplete results
  (`provenance`/`diagnostics` empty → error).
- P7.21 "suite version recorded with each run": `SUITE_VERSION` in the run
  record line.
- P7.22 "zero changes to application or domain code": the consumer
  functions are defined once and never edited by the swap (design proof in
  the test; the real `git diff` check is the P7.28 witnessed run).
- P7.22 "solver-class declarations are correct for each provider used":
  the class labels are asserted verbatim against the seam inventory §4
  declarations, and the MILP/Clarabel mismatch is asserted impossible.
- P7.23 "every attempt is denied and audited; zero attempts succeed": every
  case asserts the named refusal and (at the layers that audit) the audit
  record; DEFAULT decision 7 records the L3 audit nuance.
- Verification method items (independent witness re-runs, CI run quoted)
  are the P7.28/P7.27 human gates — not performed here.

## 9. Related records

- `04_seam_inventory.md` — seed-provider names/constructors (§3.4 of
  `07_08_lifecycle_registry.md`), solver-class declarations (§4), the
  protected non-seams (§6.1) the P7.23 attacks lean on.
- `07_08_lifecycle_registry.md` — `swap_provider` semantics and the
  named refusal order this test exercises.
- `12_13_enforcement_negatives.md` — the unit-level negatives P7.23
  composes end-to-end; the WRITTEN-UNRUN split discipline; the wasmtime
  human gate.
- `14_15_16_signing_profiles_recomposition.md` — the profile ceiling model
  (research shells) and the recomposition path the swap test exercises at
  the registry level.
- `19_20_36_config_health_path.md` — the secret-REFERENCE-only rules and
  the plugin-path identity the P7.21 determinism cases pin.
- `crates/edge-agent/tests/conformance_suite_p6_19.rs` — the suite-identity
  / case-map / claim-boundary pattern reused here (cross-referenced, not
  duplicated).
