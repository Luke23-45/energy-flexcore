# P7.07 + P7.08 — Plugin lifecycle state machine and the Class A provider registry: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P7.07, P7.08, Phase 7 ledger
(`ledger_phase_7.md`). Owner role: Rust engineer (SDK) (per ledger).
Depends on: P7.07 ← P7.05/P7.06 (manifest + resolver — satisfied, same
crate); P7.08 ← P7.04 (seam map) and P7.07. Report trace: §5.6; §5.4
Class A; §5.2; §8.1; §6.4; §9.1; §22.1 (ADR-005, ADR-010).

**Run-status honesty (AGENTS.md rule 7; the loop bans carried over from
P7.05/P7.06):** the code and tests in this wave are WRITTEN, NOT BUILT and
NOT RUN. No `cargo` command of any kind was executed (no build, no check,
no test, no clippy, no fmt). Every test below is UNRUN until the human
build/test pass executes it. Nothing in this document claims a test
passed, a build succeeded, or a compile completed. The only tools run were
the three permitted verification scripts (§9, output quoted verbatim).

## 1. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `crates/flexcore-kernel/src/lifecycle.rs` | NEW (P7.07 state machine + drain port + audit port + injected clock) |
| `crates/flexcore-kernel/src/registry.rs` | NEW (P7.08 Class A provider registry + factory port + seed constants) |
| `crates/flexcore-kernel/tests/lifecycle_p7_07.rs` | NEW (P7.07 tests) |
| `crates/flexcore-kernel/tests/registry_p7_08.rs` | NEW (P7.08 tests) |
| `crates/flexcore-kernel/src/lib.rs` | EDITED (ADDITIVE ONLY: two module-table rows + two `pub mod` lines; no existing line changed) |
| `crates/flexcore-kernel/src/manifest.rs` | EDITED (ADDITIVE ONLY: `validate_contract_id` made `pub` with a comment — the registry re-uses the manifest id grammar; logic unchanged) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.07/P7.08 ticks ONLY — P7.09/P7.40 belong to the concurrent agent and were not touched) |
| this file | NEW (decision record) |

Concurrency note honoured: `wit/**`, `plugins/wasm-examples/README.md`,
`05_06`-adjacent P7.09/P7.40 records, and ledger ticks P7.09/P7.40 belong
to the other agent; none were read for editing purposes or touched.

## 2. P7.07 — what was implemented

### 2.1 States (closed enum — the §5.6 list, mirrored exactly)

`LifecycleState` has exactly the ten states of the report §5.6 diagram:
`Discovered, Verified, Compatible, Configured, Started, Ready, Draining,
Stopped, Rejected, Failed` (`LIFECYCLE_STATES`). The enum is CLOSED:
`Stopped`, `Rejected`, and `Failed` are terminal (`is_terminal()`), and no
caller can extend the set.

### 2.2 The frozen transition table

`FROZEN_TRANSITIONS` is a constant copying the §5.6 edges verbatim; every
transition outside it is the DISTINCT named error `ILLEGAL_TRANSITION`
(never a warning, never a silent no-op). `is_legal_transition` is a pure
predicate over (frozen table ∪ crash edges); the test
`the_legal_set_is_exactly_the_frozen_table_plus_crash_edges` enumerates all
100 state pairs and pins the predicate to the tables.

| # | From | To | Guard (§5.6 label) | Enforced as |
|---|---|---|---|---|
| 1 | Discovered | Verified | signature / provenance accepted | `verify(GATE_PROVENANCE)` |
| 2 | Verified | Compatible | manifest and API compatibility pass | `mark_compatible(GATE_MANIFEST_API_COMPATIBILITY)` |
| 3 | Compatible | Configured | schema and secret references valid | `configure(GATE_SCHEMA_SECRETS)` |
| 4 | Configured | Started | dependency graph resolved | `start(GATE_DEPENDENCY_GRAPH)` — the P7.06 plan is the real evidence |
| 5 | Started | Ready | health checks pass | `mark_ready(GATE_HEALTH)` |
| 6 | Ready | Draining | disable / upgrade requested | `begin_drain` |
| 7 | Draining | Stopped | active work completed or cancelled | `complete_drain` (via the `DrainPort`) |
| 8 | Verified | Rejected | verification failure | `reject` |
| 9 | Compatible | Rejected | incompatible API | `reject` |
| 10 | Started | Failed | initialization failure | `fail` |
| 11 | Ready | Failed | health / runtime failure | `fail` |
| 12 | Failed | Draining | failed plugin drained before removal | `begin_drain` (from `Failed`) |

### 2.3 Gates are handed in, not re-implemented

The lifecycle does not verify signatures or run health checks — the §5.6
gate WORK is owned by the pieces that exist: P7.14 (signing, future) at
`Verified`, the P7.05 validator + §9.3 range check at `Compatible`, the
P7.05 secret-reference rule at `Configured`, the P7.06 resolver at
`Started`. The machine enforces the OUTCOME: each forward step takes a
`GateOutcome` (`Passed` / `Failed { reason }`); a failed gate REFUSES the
transition (fail-closed, state unchanged), audits the refusal as an
attempted edge, and the §5.6 exit edge (`Rejected`/`Failed`) is then
applied explicitly by `reject`/`fail`. This keeps the frozen table exact —
no invented edges — while making every gate failure visible.

**DEFAULT decision (gate failure at `Discovered`):** §5.6 has no
`Discovered → Rejected` edge, so a provenance failure at the very first
gate leaves the plugin in `Discovered` (refused + audited; it simply never
advances) — the P7.14 step "an unsigned or bad-signature package is
`Rejected` before any compatibility check" will be able to map onto
`Discovered → Verified` refused + operator-applied rejection when the
signing wave lands; recorded here so the shape is already in writing.

### 2.4 Drain-before-replace (the core rule)

- `Ready → Draining` via `begin_drain`, which REQUIRES the plugin's
  `DrainPort` (a trait the plugin implements: `outstanding_work()` and
  `drain(deadline)`). A plugin with no port is refused up front
  (`DRAIN_PORT_MISSING`) — a plugin that cannot report and abandon its
  outstanding work is never assumed quiet.
- `complete_drain` runs the port against the configured deadline:
  `Quiesced { cancelled }` commits `Draining → Stopped`;
  `DeadlineExceeded { outstanding }` REFUSES the stop (fail-closed),
  emits a `REFUSED` audit record for the attempted `Draining → Stopped`
  edge, RESTORES the incumbent to `Ready` (active) with an audited
  restoration record, and returns `DRAIN_DEADLINE_EXCEEDED`. There is no
  forced stop: **the operator decides**.
- **Deadline policy (DEFAULT decision):** `DEFAULT_DRAIN_DEADLINE_MS =
  30_000` (30 seconds of injected-clock time), overridable per plugin via
  `DrainPolicy`. The report gives no number; 30 s bounds a
  schedule-length drain without hiding a stuck plugin for minutes.
- **Replacement gate:** `require_fully_drained` allows a replacement ONLY
  from `Stopped`. From `Ready` (active, no drain begun) or `Draining`
  (in progress, not quiesced) it returns the NAMED error
  `REPLACEMENT_BLOCKED_UNDRAINED` and audits the refusal — never a
  warning. The registry's `swap_provider` (P7.08) is the one that calls
  it.
- **Version-change recording:** after a completed drain,
  `record_version_change(new_version)` (legal only from `Stopped`) emits
  the `VERSION_CHANGE` audit record carrying the old version and the new
  one in its reason — P7.07 step 3: "record the version change, then
  replace".
- **No activation mid-dispatch:** the machine structurally cannot
  activate over an un-drained incumbent (the replacement gate), and the
  registry structurally cannot swap without the lifecycle gate. The
  ledger's acceptance item "an upgrade attempted during an active
  dispatch transaction is refused (or queued) — record the chosen
  behaviour": the chosen behaviour is REFUSED (fail-closed; the operator
  retries after the drain), recorded here and in the swap tests. A
  dispatch-transaction-level deferral (queueing) is a P7.16 decision and
  is deliberately not half-built here.

### 2.5 Crash handling and the EXPLICIT-only restart policy

- `crash()` enters `Failed` from ANY non-terminal state
  (`CRASH_TRANSITIONS`: Discovered/Verified/Compatible/Configured/
  Draining — Started and Ready already have §5.6 `fail` edges). **DEFAULT
  decision:** the §5.6 diagram names `Failed` exits from `Started`/`Ready`
  only; a crash in any other state must still be representable and
  audited (a crash that could not be recorded would look healthy). The
  frozen table itself is unchanged.
- The failed lifecycle RETAINS the state it failed from
  (`failed_from()`), held as evidence for the P7.30 failure-mode drill
  (the ledger's risk: "a `Failed` plugin that never drains and blocks the
  registry" — the cleanup path is the §5.6 `Failed → Draining` edge,
  tested).
- **Restart policy = EXPLICIT ONLY in v0.5.0 (DEFAULT decision, recorded):
  no auto-restart exists anywhere in this module.** The only path out of
  `Failed` besides `Failed → Draining` is `restart_explicitly()`, which is
  refused from any other state, audited as an `EXPLICIT_RESTART` record,
  and returns a fresh lifecycle at `Discovered`. The test
  `the_only_restart_path_is_explicit_and_audited_and_there_is_no_auto_restart`
  proves the negative case (a gated forward step out of `Failed` is
  illegal).

### 2.6 Audit hooks

Every transition — committed OR refused — emits a `LifecycleAuditRecord`
through the in-module `AuditSink` port: plugin id, version, from/to
states, kind, actor, reason, and a timestamp from the INJECTED `Clock`.
`SystemTime::now()` appears nowhere in the crate (P7.42 determinism;
`FixedClock` is the deterministic implementation; the wall-clock adapter
is injected at the composition root — the `crates/edge-agent/src/clock.rs`
pattern, narrowed to what the lifecycle needs).

**Recorded decision (in-module port):** the `AuditSink`/`Clock` ports are
DEFINED IN `lifecycle.rs`, not in `flexcore-ports`, because the kernel
depends on no `flexcore-*` crate (the P7.05/P7.06 dependency decision in
the crate's `Cargo.toml`), and because the name `AuditSink` already
collides twice in the platform (the seam inventory P7.04 §3.8 records
`flexcore-audit::AuditSink` vs the edge-local `edge-agent::ports::AuditSink`
— this is a third, kernel-local port feeding the host's audit path, named
`lifecycle::AuditSink` and used only for lifecycle records). A composition
root bridges it into the real audit pipeline.

### 2.7 Named lifecycle errors

| Name (`LifecycleError::name`) | Meaning |
|---|---|
| `ILLEGAL_TRANSITION` | the attempted edge is not in the frozen table (nor the crash set) |
| `GATE_REFUSED` | a §5.6 gate outcome was not `Passed`; state unchanged, refusal audited |
| `DRAIN_PORT_MISSING` | a drain was requested with no `DrainPort` attached |
| `DRAIN_DEADLINE_EXCEEDED` | the drain deadline expired; the stop was refused, the incumbent restored to Ready |
| `REPLACEMENT_BLOCKED_UNDRAINED` | a replacement was attempted while the incumbent was not fully drained |

## 3. P7.08 — what was implemented

### 3.1 Registry design (§5.4 Class A; §6.4 Factory/Registry)

`ProviderRegistry<T: ?Sized>` holds provider FACTORIES per contract id,
generic over the contract trait-object type. Key properties:

- **Registered at the composition root from a `CompositionPlan`** (the
  P7.06 resolver output): `new(&plan)` binds the plan's resolved contract
  table; `register_from_plan` registers the batch — requiring a factory
  for EVERY plan contract (`FACTORY_MISSING_FOR_PLAN_CONTRACT`) and
  refusing any STRAY factory not named by the plan
  (`CONTRACT_NOT_IN_PLAN`). Incremental `register` is gated by the same
  plan table. A populated registry is never re-planned in place
  (`REGISTRY_ALREADY_POPULATED`) — re-composition (P7.16) builds a fresh
  registry.
- **No global mutable registry / service locator (§6.4 "avoid"):** there
  is no `static`, no `lazy_static`, no thread-local anywhere in the
  module; the registry is a composition-root-owned value. Construction is
  on demand (`construct`), the registry stores factories, never live
  providers.
- **Duplicates refused:** `DUPLICATE_CONTRACT_REGISTRATION` — never a
  silent overwrite; the test proves the original factory still answers.
- **Determinism:** the factory table is a `BTreeMap`;
  `contract_ids()` iterates sorted (P7.42); error selection order is
  deterministic.

### 3.2 Protected-kernel reinforcement and the reserved-id reconciliation

The registry independently refuses ANY factory under a protected-kernel
contract id (`PROTECTED_KERNEL_CONTRACT_REFUSED`, checked FIRST — before
id grammar, plan membership, and duplicates; and again in
`swap_provider`), reusing `resolve::PROTECTED_KERNEL_CONTRACT_IDS` as the
single source of the list.

**Reserved-id reconciliation note (P7.06 list vs seam inventory §6.1):**
the P7.06 resolver hardcoded six ids before P7.04 landed, with the comment
that the list would be reconciled against the seam record. P7.04 §6.1
names exactly the same six protected non-seams — `CommandAuthorizer`,
`DispatchGateway`, tenant isolation, audit integrity, command lifecycle,
final dispatch validation — mapped one-to-one onto the six ids
(`kernel.command-authorizer`, `kernel.dispatch-gateway`,
`kernel.tenant-isolation`, `kernel.audit-integrity`,
`kernel.command-lifecycle`, `kernel.dispatch-validation`). The two lists
ALIGN; the union is the same six ids. The P6.14 edge command-path gate is
NOT a contract id (no seam exists for it — P7.04 §6.1 last row: the
never-granted capability set implicitly includes the gate), so it adds no
id. No list change was needed.

### 3.3 The lifecycle-aware swap (the P7.22 surface, not the P7.22 test)

`swap_provider(contract_id, factory, lifecycle, new_version, actor,
reason)` is the ONLY path the P7.22 swap test will use. Refusal order:
protected kernel → contract known (`SWAP_CONTRACT_UNKNOWN`) → lifecycle
identity (`SWAP_LIFECYCLE_MISMATCH`: the handle must belong to the
registered provider plugin of the contract) → drain state (the lifecycle's
`require_fully_drained`; `LIFECYCLE_REFUSED` wrapping
`REPLACEMENT_BLOCKED_UNDRAINED`) → version-change record. On success the
factory is replaced and the version change is audited through the
lifecycle. The swap test itself (P7.22's wave: the §23 Stage 7 exit-gate
rehearsal with equivalence criteria) is deliberately NOT implemented here.

### 3.4 Seed-provider inventory (real names; what is and is not registrable in-test)

The REAL first-party providers, by the names the seam inventory uses
(P7.04 §3, grep-verified there), with their ACTUAL constructors (read from
source; none invented):

| Provider (real name) | Contract seam | Constructor (verified) | Runtime config the factory captures | Registrable in-test inside the kernel? |
|---|---|---|---|---|
| `HighsTier1Provider` (`flexcore-optimization-api/src/highs_provider.rs`) | `Optimizer` (§3.1) | `new(backend: Arc<dyn Tier1Backend>)` | the tier-1 solver backend | NO — the kernel depends on no `flexcore-*` crate, and the backend is runtime config |
| `ClarabelTier2Provider` (`src/clarabel_provider.rs`) | `Optimizer` (§3.1) | `new(config: Tier2ProviderConfig)` | the provider config (solver-class declarations per §7.4/§9.2 ride `SolverCapabilities`) | NO (same reasons) |
| `PandapowerGridProvider` (`flexcore-grid-api/src/adapter.rs`) | `GridModelProvider` (§3.2) | `new(backend, scope)` (backend + worker scope, as `apps/sim-runner` constructs it) | the `GridModelBackend` + scope | NO (same reasons) |
| `SimulatorTelemetrySource` (`flexcore-simulator/src/source.rs`) | `TelemetrySource` (§3.4) | `new(...)` with runtime parameters | its source configuration | NO (same reasons) |

Each factory takes its runtime config as a CAPTURED PARAMETER of an
`FnFactory` closure at the composition root — no constructor was invented
inside any other crate, and no NEW direct import of a concrete provider
type was added anywhere (the seam-inventory audit note G-2 is respected:
nothing in this wave touches that borderline constant import).

**DEFAULT decision (seed contract/plugin ids):** the seam inventory names
the contract TRAITS, not contract ids; the registry declares seed ids
`optimization.solver` / `grid.model` / `telemetry.source` and default
plugin ids `builtin.highs-optimizer` / `builtin.pandapower-grid` /
`builtin.simulator-telemetry` (report §8.1 `plugins/builtin/` naming
intent: asset simulator, HiGHS optimizer, market mock). These are
proposals for the P7.15 profile work to ratify, labelled DEFAULT in
`registry::seed` and here. The registry refuses to seed itself without a
plan — the ids only bind when a resolved plan contains them.

**In-test honesty:** the registry tests use LOCAL test factories over a
trivial local trait (`Solver`), named after the real providers via the
seed constants — exactly the recorded fallback ("otherwise local test
factories implementing a trivial local trait"). The real registration is a
composition-root line through the same `FnFactory` adapter (usage shown in
the `FnFactory` doc comment).

### 3.5 Named registry errors

| Name (`RegistryError::name`) | Meaning |
|---|---|
| `PROTECTED_KERNEL_CONTRACT_REFUSED` | a factory under a protected-kernel contract id (checked first) |
| `CONTRACT_ID_MALFORMED` | not a valid namespaced contract id (manifest grammar, re-used) |
| `CONTRACT_NOT_IN_PLAN` | a provider for a contract the plan does not resolve |
| `FACTORY_MISSING_FOR_PLAN_CONTRACT` | `register_from_plan` lacked a factory for a plan contract |
| `REGISTRY_ALREADY_POPULATED` | re-planning a populated registry in place |
| `DUPLICATE_CONTRACT_REGISTRATION` | a second factory for one contract |
| `CONTRACT_NOT_REGISTERED` | construction requested for an unregistered contract |
| `CONSTRUCTION_FAILED` | a factory refused to build its provider |
| `SWAP_CONTRACT_UNKNOWN` | swap named an unregistered contract |
| `SWAP_LIFECYCLE_MISMATCH` | the lifecycle handle belongs to a different plugin |
| `LIFECYCLE_REFUSED` | the P7.07 lifecycle refused the swap (source name preserved) |

## 4. Acceptance-criteria mapping (ledger)

| Ledger item | Where |
|---|---|
| P7.07: every §5.6 transition tested incl. `Rejected`/`Failed` exits | `lifecycle_p7_07.rs`: happy path (edges 1–7), gate-refusal → `reject` (edge 8/9 shape), `fail` (10/11), `Failed → Draining` (12) |
| P7.07: upgrade during an active dispatch transaction refused and audited; behaviour recorded | §2.4 (REFUSED, fail-closed); `drain_before_replace_blocks_the_replacement_until_quiescence`, `swap_is_blocked_while_active_and_allowed_after_drain` |
| P7.07: a version change is recorded | `record_version_change` + the swap test's `VERSION_CHANGE` audit assertion |
| P7.07: random transition sequences never reach an undefined state | the closed state space + `the_legal_set_is_exactly_the_frozen_table_plus_crash_edges` (full 10×10 enumeration); the property fuzzer itself is P7.42's wave |
| P7.08: all built-ins register through the registry; no direct construction in application code | the seed inventory (§3.4) + `FnFactory`; the existing composition roots (`apps/sim-runner`) are untouched by this wave — the grep re-run is the first-build verification step |
| P7.08: swap test passes | NOT CLAIMED — the swap MECHANISM (`swap_provider`) is implemented and unit-tested here; the P7.22 witnessed swap test is that wave's deliverable (UNRUN here) |
| P7.08: no global mutable registry (grep + inspection) | no `static`/`lazy_static`/thread-local in `src/registry.rs` or `src/lifecycle.rs`; re-grep at first build |

## 5. DEFAULT decisions (one list)

1. Crash edges from every non-terminal state (§2.5) — extension of, not
   change to, the frozen §5.6 table.
2. Restart policy EXPLICIT only; no auto-restart (§2.5).
3. Drain deadline 30 s, overridable (§2.4).
4. Deadline expiry → stop refused, incumbent restored to Ready, operator
   decides (§2.4).
5. Upgrade/swap during active work: REFUSED, not queued (§2.4); queueing
   is a P7.16 decision.
6. Gate failure at `Discovered` leaves the plugin undiscovered-advanced,
   refused + audited (§2.3).
7. In-module `AuditSink`/`Clock` ports (kernel depends on no `flexcore-*`
   crate; naming-collision note) (§2.6).
8. Seed contract/plugin ids (`registry::seed`) — proposals for P7.15 to
   ratify (§3.4).
9. `register_from_plan` requires full plan coverage in BOTH directions
   and refuses a populated registry (§3.1).

## 6. Related records

- `04_seam_inventory.md` — the seed-provider names and the protected
  non-seams (§6.1) reconciled in §3.2; the G-2 borderline import note.
- `05_06_manifest_resolution.md` — the manifest/resolver types consumed
  here (`Manifest`, `CompositionPlan`, `PROTECTED_KERNEL_CONTRACT_IDS`,
  `CURRENT_PLATFORM_API_VERSION`).
- `02_scope_freeze.md` — Class A definition (§6) and the never-granted
  capability set the lifecycle never bypasses.

## 7. Related subtasks consuming this wave (skimmed, not touched)

- P7.15/P7.16 (profiles + re-composition) consume the lifecycle's drain
  and the registry's plan-bound registration; P7.30 (failure drill)
  consumes `failed_from()` and the crash records; P7.22 uses
  `swap_provider`; P7.14 plugs into the `GATE_PROVENANCE` outcome at
  `Verified`.

## 8. Status

IMPLEMENTED (written), NOT BUILT, tests NOT RUN — UNRUN until the human
build/test pass. No `todo!()`/`unimplemented!()`/TODO markers; no
`unwrap`/`expect` on untrusted input in `src/` (test files may unwrap).

## 9. Verification tools (permitted only; output verbatim)

See the wave report accompanying this record: `tools/verify_docs.py`,
`tools/validate_agent.py`, and `python -I tools/check_dependency_rule.py`
(the dependency checker must print OK for 18 members). No cargo command
was run.
