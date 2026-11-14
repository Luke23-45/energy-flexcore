# Lifecycle, gates, drain-before-replace, and profiles

Sources of record: `crates/flexcore-kernel/src/lifecycle.rs` (P7.07),
`crates/flexcore-kernel/src/recomposition.rs` (P7.16),
`crates/flexcore-kernel/src/profile.rs` (P7.15), report §5.6/§5.8/§21.4.

## 1. The §5.6 states

The lifecycle is a closed enum mirroring the report diagram exactly
(`lifecycle.rs::LifecycleState`), with the machine names the audit records
and the console render verbatim:

| State | Machine name | Meaning |
|---|---|---|
| Discovered | `DISCOVERED` | registered; nothing verified yet |
| Verified | `VERIFIED` | signature / provenance accepted |
| Compatible | `COMPATIBLE` | manifest and API compatibility pass |
| Configured | `CONFIGURED` | schema and secret references valid |
| Started | `STARTED` | dependency graph resolved |
| Ready | `READY` | health checks pass — the only serving state |
| Draining | `DRAINING` | stopped accepting new work; finishing/cancelling outstanding operations |
| Stopped | `STOPPED` | fully drained (terminal) |
| Rejected | `REJECTED` | verification or compatibility failure (terminal) |
| Failed | `FAILED` | initialization/health/runtime failure (terminal) |

## 2. The frozen transition table and the gates

The twelve §5.6 edges are a frozen constant (`FROZEN_TRANSITIONS`); five
additional `→ Failed` crash edges (from `Discovered`, `Verified`,
`Compatible`, `Configured`, `Draining`) exist so a crash is always
representable — a crash that could not be recorded would look healthy.
Every transition outside the tables is the named error `ILLEGAL_TRANSITION`
— never a warning, never a silent no-op.

The five §5.6 gate preconditions are enforced as required
[`GateOutcome::Passed`] values on the forward transitions; the HOST runs
the checks and hands the outcome in, and the machine refuses to advance on
anything else:

| Gate name | Transition | Checks |
|---|---|---|
| `PROVENANCE` | `Discovered → Verified` | signature / provenance record accepted (P7.14) |
| `MANIFEST_API_COMPATIBILITY` | `Verified → Compatible` | manifest validity + api-version coverage |
| `SCHEMA_SECRETS` | `Compatible → Configured` | configuration schema + secret references (P7.19) |
| `DEPENDENCY_GRAPH` | `Configured → Started` | composition resolved (P7.06 plan) |
| `HEALTH` | `Started → Ready` | health checks pass (P7.20) |

A failed gate refuses the transition (state unchanged) and is audited; the
§5.6 exit edges (`Verified/Compatible → Rejected`, `Started/Ready →
Failed`) are then applied explicitly.

## 3. Drain-before-replace (the core rule)

A plugin being replaced moves `Ready → Draining`, stops accepting new work,
and finishes or cancels outstanding operations through its `DrainPort`
before it may reach `Stopped`. A replacement may only start once the
incumbent is fully drained (`Stopped`); anything earlier is the named
error `REPLACEMENT_BLOCKED_UNDRAINED` — never a warning.

A drain that hits its deadline is REFUSED (fail-closed): the
`Draining → Stopped` transition is NOT committed, the incumbent is
restored to `Ready`, and the refusal (`DRAIN_DEADLINE_EXCEEDED`, with the
outstanding count) is audited — the operator decides. There is no forced
stop. A drain requested from a plugin with no `DrainPort` attached is
`DRAIN_PORT_MISSING` (a plugin that cannot report its outstanding work is
never assumed quiet). The default drain deadline is 30 seconds
(`DEFAULT_DRAIN_DEADLINE_MS`; DEFAULT decision, overridable per
composition via `DrainPolicy`).

Every transition — committed or refused — emits a `LifecycleAuditRecord`
(kind `REGISTRATION` / `TRANSITION` / `REFUSED` / `VERSION_CHANGE` /
`EXPLICIT_RESTART`, with plugin id/version, actor, reason, and a timestamp
from the injected `Clock`) through the kernel-local `AuditSink` port.

## 4. Crash handling and restart (EXPLICIT only)

A plugin that fails STAYS `Failed` (crash edges cover every non-terminal
state). The v0.5.0 restart policy is EXPLICIT ONLY (DEFAULT decision):
there is no auto-restart anywhere; the only path out of `Failed` is the
audited explicit restart, which starts a fresh lifecycle at `Discovered`.
The failed lifecycle retains the state it failed from as evidence.

## 5. The five profiles (§5.8)

A profile IS DATA (`flexcore-profile-v1`), never code: an ordered selection
list, a permission ceiling, and the control-authority posture — everything
visible in its file, no hidden defaults. The shipped five
(`plugins/profiles/`, loader `profile.rs`):

| Profile | v0.5.0 state | Ceiling |
|---|---|---|
| `sim-lab` | loads; composes the three seed built-ins (+ `neryva.sim-builtin-ess`) | `telemetry.read`, `asset.read`, `schedule.propose` |
| `vpp-operator` | loads; composes the three seed built-ins | the full allowed §5.7 vocabulary (no never-granted ids) |
| `site-edge` | DEFERRED SHELL — loads, composes NOTHING (`PROFILE_EMPTY_SELECTIONS` at plan validation) | `telemetry.read`, `asset.read` |
| `research-gnn` | loads; composes the grid-model and telemetry seeds | `telemetry.read`, `asset.read` ONLY — the proposal permissions are withheld (no dispatch authority) |
| `research-marl` | same state as `research-gnn` | `telemetry.read`, `asset.read` ONLY |

Unresolvable selections ship COMMENTED OUT in the profile JSON (the
documented pattern; the loader strips full-line `//` comments only).
Enabling a deferred selection is an EDIT to the profile file — a reviewed,
audited change, never a runtime flag. The edge "limited command receiver"
has no entry at all — not even a commented-out one — because it is
protected kernel (P6.29 §3): no profile can express, reference, or smuggle
it.

The ceiling restricts; it never grants. Every selected manifest's
`permissions` must be a SUBSET of the ceiling (`PROFILE_PERMISSION_CEILING_EXCEEDED`),
and each selection pins every contract in its manifest's `provider-of` as
an explicit selection — provider conflicts are resolved IN the reviewable
profile data, never silently (`PROFILE_DUPLICATE_CONTRACT_PIN`).

## 6. Profile switching with confirmations (P7.16)

Explicit, audited profile switching (`recomposition.rs`): the target
profile re-resolves through the P7.06 resolver, and the switch is a
deterministic plan diff (`PREPARE → DRAIN → REPLACE → REMOVE → ADD` steps)
executed only against an explicit `SwitchConfirmation` — a switch without
one is `SWITCH_CONFIRMATION_MISSING`, a mismatched one is
`SWITCH_CONFIRMATION_MISMATCH`. Other named refusals:

| Error name | Meaning |
|---|---|
| `CONTROL_AUTHORITY_MODE_CHANGE_REFUSED` | a switch handed a control-authority change is refused at the composition boundary (defence in depth: profile DATA can only ever carry `simulation-shadow`, `CONTROL_AUTHORITY_MODE_FORBIDDEN` at parse) |
| `PROTECTED_CONTRACT_IN_TARGET` | the target composition claims a protected contract |
| `SWITCH_NO_OP_REFUSED` | a switch that changes nothing is refused (not silently acknowledged) |
| `SWITCH_STEP_REFUSED` | a step failed; partial-failure semantics are recorded with the restart fallback |
| `PROFILE_RESOLUTION_REFUSED` | the target profile does not resolve (wrapping the resolver's named error — e.g. `MISSING_PROVIDER`, `CYCLIC_DEPENDENCY`, `PROTECTED_KERNEL_REPLACE`) |

The control-authority restatement (§21.4) is part of every switch audit
(subject `composition.profile-switch`): a profile switch can never change
the control-authority mode.

## 7. Operator-facing consequence

An operator reading the console (the Plugins & Integrations workspace,
P7.25) sees each plugin's lifecycle state verbatim from the §1 machine
names, the active profile's composition, and the recorded confirmations.
The console itself can never EXECUTE a switch: its profile-switch
affordance records a REQUEST only (a local console event), exactly like
the edge console's command-path rule.
