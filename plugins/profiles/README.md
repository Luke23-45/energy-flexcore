# Composition profiles — `flexcore-profile-v1`

Shipped profile definitions for the v0.5.0 plugin SDK (P7.15; report §5.8;
PLAN.md §1). A profile **is data, never code**: a versioned JSON document
naming an ordered list of plugin selections, a permission ceiling, and the
control-authority posture. There is no code path behind a profile and no
hidden default — everything a profile does is visible in its file, which is
the counter to the ledger's recorded risk ("a profile that is really a code
path with hidden defaults").

| Field | Value |
|---|---|
| Schema id | `flexcore-profile-v1` |
| Version | 1.0.0 (the schema itself is SemVer'd) |
| Subtask | P7.15 (Phase 7, `ledger/phase-07-plugin-sdk/ledger_phase_7.md`) |
| Report trace | §5.8 (named runtime profiles), §5.1/§5.7 (protected kernel, permission vocabulary), §21.4 (control-authority modes) |
| Loader/validator | `crates/flexcore-kernel/src/profile.rs` (`flexcore-kernel` crate) |
| Status | Implemented, awaiting human build/test (loop bans; decision record `ledger/phase-07-plugin-sdk/14_15_16_signing_profiles_recomposition.md`) |

## 1. The five shipped profiles

| Profile | Content (§5.8) | v0.5.0 state |
|---|---|---|
| `sim-lab` | Synthetic DERs, feeder simulator, mock market, deterministic optimizer, replay tools; no device writes | Loads; composes the three seed built-ins; mock-market / replay / ESS entries deferred (commented out) |
| `vpp-operator` | Registry, telemetry, flexibility, scheduler, audit, console, selected site connectors | Loads; composes the three seed built-ins; forecast/WASM examples and console/scheduler/connector entries deferred |
| `site-edge` | Edge agent, selected protocol adapters, local buffering, outbound identity, limited command receiver | **Deferred shell** — loads and is reviewable, composes NOTHING (`PROFILE_EMPTY_SELECTIONS` at plan validation); ledger P7.15 step 4: "record which profiles are deferred and why" |
| `research-gnn` | Grid model provider, worker, benchmark suite, shadow logger; **no dispatch authority** | Loads; composes the grid-model and telemetry seeds; the research worker itself is Phase 8 (PLAN.md §9) |
| `research-marl` | Same shape as `research-gnn`, for the MARL line; **no dispatch authority** | Same state as `research-gnn` |

## 2. The commented-out-entries pattern

A profile that cannot fully resolve in v0.5.0 ships with its unresolvable
selections **commented out in the JSON**. The loader strips FULL-LINE `//`
comments only (a line whose first non-whitespace characters are `//`) before
JSON parsing — a deterministic, documented JSONC-style extension; `//`
inside a string value is never affected and there are no trailing or block
comments. Every commented block carries its reason inline. Consequences:

- A commented selection is NOT loaded, NOT validated, and NEVER composed —
  it is documentation of intent, nothing more.
- Enabling a deferred selection is an EDIT to the profile file: a reviewed,
  audited change (never a runtime flag) — the ledger's "enabling a
  control-capable plugin requires an explicit, reviewed profile change".
- `site-edge` composes nothing in v0.5.0 (empty active selections): it is
  refused at plan validation with `PROFILE_EMPTY_SELECTIONS` by design.

## 3. The control-authority posture is locked

Every shipped profile carries `"control-authority": "simulation-shadow"`.
The field's vocabulary is CLOSED and the loader accepts EXACTLY that one
value: any other value — including the other §21.4 modes
(`read-only-shadow`, `operator-approved`, `bounded-automation`) — is the
named error `CONTROL_AUTHORITY_MODE_FORBIDDEN`. This is the §5.8/§21.4 rule
made structural: **the control-authority mode is a deployment/per-site
setting; a profile, like a plugin, can never select or raise it.** P7.16's
re-composition restates the same guarantee at the switch boundary.

## 4. The permission-ceiling model

`permission-ceiling` is the MAXIMUM set of §5.7 permission ids any plugin in
the profile may carry. Rules, all enforced with named errors:

- Ceiling entries come from the §5.7 allowed vocabulary only
  (`telemetry.read`, `asset.read`, `forecast.publish`, `schedule.propose`,
  `dispatch.propose`, `dispatch.validate.request`); anything else is
  `PROFILE_PERMISSION_CEILING_UNKNOWN`.
- The never-granted set (`dispatch.authorize`, `dispatch.issue`,
  `secret.read`, `network.any`) cannot appear in a ceiling
  (`PROFILE_PERMISSION_CEILING_NEVER_GRANTED`) — a profile can never
  ceiling what no manifest may request.
- At plan validation, every selected manifest's `permissions` must be a
  SUBSET of the ceiling, else `PROFILE_PERMISSION_CEILING_EXCEEDED`. The
  ceiling restricts; it never grants.
- `research-gnn` / `research-marl` withhold even the proposal permissions
  (`schedule.propose`, `dispatch.*`): their ceiling is
  `telemetry.read` + `asset.read` only. This is the recorded enforcement of
  "no dispatch authority" for the research profiles (DEFAULT decision;
  ADR-010 / §13.4; P7.31 handover may revisit under the proposal-only
  boundary). Attempting a dispatch from within them is structurally
  refused: no plugin in the composition can hold a dispatch-side
  permission, and the never-granted set is refused one layer earlier at
  every manifest gate.

## 5. Manifest references

Selections reference manifests by plugin id (`manifest-id`) plus a version
constraint parsed as a comparator range (never string-compared — the P7.05
grammar). At plan validation, every referenced manifest must: exist in the
supplied manifest set (`PROFILE_SELECTION_UNRESOLVABLE` otherwise), satisfy
the constraint (`PROFILE_MANIFEST_VERSION_UNSATISFIED`), stay under the
ceiling, and resolve through the P7.06 resolver into a cycle- and
conflict-free plan (`PROFILE_RESOLUTION_REFUSED` wrapping the resolver's
named error — e.g. `MISSING_PROVIDER`, `CYCLIC_DEPENDENCY`,
`PROTECTED_KERNEL_REPLACE`). Each selection pins every contract in its
manifest's `provider-of` as an EXPLICIT provider selection; two selections
pinning one contract is `PROFILE_DUPLICATE_CONTRACT_PIN`, never a silent
last-wins.

The active selections reference the seed built-in ids
(`builtin.simulator-telemetry`, `builtin.highs-optimizer`,
`builtin.pandapower-grid`) — the DEFAULT-labelled seed ids proposed in
P7.08's `registry::seed` and ratified here by the P7.15 profile work. They
resolve when the corresponding manifests are supplied at composition time.

### 5.1 Deferred entries and their reasons (why commented out)

- `neryva.sim-builtin-ess` — the example Class A ESS manifest exists, but
  its `api-version` (`>=0.5,<1.0`) uses two-component comparator versions,
  which the v1 manifest grammar rejects (versions are strict
  `MAJOR.MINOR.PATCH` cores). The entry stays commented out until the
  example manifest is corrected. (Observed inconsistency between the
  published schema document's example and the validator's grammar;
  recorded in the decision record — not silently "fixed" here.)
- `acme.forecast-worker`, `neryva.optimizer-wasm-lp` — the illustrative
  SDK examples; each consumes a contract (`telemetry.history`,
  `portfolio.read`) that no v0.5.0 manifest provides, so a profile
  containing them cannot resolve.
- `builtin.mock-market`, `builtin.replay-tools`, console/scheduler/
  connector ids — no plugin manifests exist for these roles yet.
- Protocol adapters, local buffering, outbound identity, edge agent —
  deferred or non-plugin seams (02_scope_freeze §3; seam inventory §3.10).
- The edge "limited command receiver" — **protected kernel** (P6.29 §3).
  It has no entry at all, not even a commented-out one: no profile can
  express, reference, or smuggle it.

## 6. Named-error summary

`PROFILE_JSON_INVALID`, `PROFILE_NOT_AN_OBJECT`, `PROFILE_SCHEMA_UNSUPPORTED`,
`PROFILE_UNKNOWN_FIELD`, `PROFILE_MISSING_FIELD`, `PROFILE_WRONG_TYPE`,
`PROFILE_ID_MALFORMED`, `PROFILE_VERSION_CONSTRAINT_INVALID`,
`PROFILE_SELECTION_MANIFEST_ID_MALFORMED`, `PROFILE_DUPLICATE_SELECTION`,
`PROFILE_SELECTION_COUNT_EXCESSIVE`, `PROFILE_PERMISSION_CEILING_UNKNOWN`,
`PROFILE_PERMISSION_CEILING_NEVER_GRANTED`,
`PROFILE_PERMISSION_CEILING_DUPLICATE`, `CONTROL_AUTHORITY_MODE_FORBIDDEN`,
`PROFILE_EMPTY_SELECTIONS`, `PROFILE_SELECTION_UNRESOLVABLE`,
`PROFILE_MANIFEST_VERSION_UNSATISFIED`, `PROFILE_PERMISSION_CEILING_EXCEEDED`,
`PROFILE_PROTECTED_CONTRACT_REFUSED`, `PROFILE_DUPLICATE_CONTRACT_PIN`,
`PROFILE_RESOLUTION_REFUSED`.

## 7. Honest status

These files and the loader are WRITTEN, NOT BUILT and NOT RUN — the loop
bans hold; the human build/test pass runs
`cargo test -p flexcore-kernel --test profile_p7_15` and the other suites.
Nothing in this README claims a load or composition succeeded.
