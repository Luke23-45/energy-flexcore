# SDK scenario-regression suite (`plugins/scenarios/`)

The P7.43 scenario-regression suite for the plugin SDK (report §22.2; ledger
`ledger_phase_7.md` §P7.43). Each JSON file in this directory is ONE named
scenario: pinned inputs (embedded manifests, profiles, config documents),
recorded versions and seeds, and the expected outcome of every step as a
NAMED verdict. The suite is executed by the in-test interpreter in
`crates/flexcore-kernel/tests/scenario_regression_p7_43.rs`, which embeds
these files at compile time (`include_str!`) — the CI job
`sdk-scenario-regression` in `.github/workflows/ci.yml` runs that test target
and a failing scenario blocks the merge.

This is the SDK leg of the platform scenario-regression pattern (P5.20
`scenarios/library/` + P6.27 edge wiring). It shares the PATTERN, not the
scenarios: no scenario is duplicated across legs, and no scenario here
re-runs a unit suite — each step calls a public kernel API with the
scenario's pinned inputs.

## Status

The suite and interpreter are WRITTEN, NEVER RUN (the delivery loop bans
hold). The green CI run is a human gate input; no pass is claimed anywhere.

## File format

```jsonc
{
  "scenario-id": "sdk-01-manifest-to-plan",     // unique, stable scenario name
  "scenario-version": "1.0.0",                  // bumped when the scenario changes
  "suite": "flexcore-plugin-sdk-scenario-regression-v1",  // the suite id CI logs
  "description": "...",
  "pins": {
    "seeds": "none | <recorded seed constant and its derivation>",
    "versions": { ... },                        // schema ids / pinned constants the scenario depends on
    "tolerance": "exact — ..."                  // the comparison rule for every step
  },
  "inputs": {
    "manifests": [ { ...flexcore-plugin-manifest-v1... } ],
    "profiles":  [ { ...flexcore-profile-v1... } ],
    "configs":   [ { ...arbitrary JSON value... } ]   // referenced by "config-index"
  },
  "steps": [ { "step": "<kind>", ..., "expect": { ... } } ]
}
```

`expect` is one of:

- `{ "outcome": "ok" }` — the call succeeds;
- `{ "outcome": "ok", "registrations": [ids] }` — plan registration order pinned;
- `{ "outcome": "ok", "counts": { "added": n, "removed": n, "replaced": n, "departing": n, "arriving": n, "upgraded": n } }` — plan-diff counts pinned;
- `{ "outcome": "ok", "applied-steps": n }` — switch step count pinned;
- `{ "outcome": "ok", "resolved-json": {...} }` — resolved config view pinned;
- `{ "outcome": "ok", "variant": "PROPOSAL_RECORDED" | "RESULT_OBSERVED" }` — path outcome pinned;
- `{ "outcome": "refused", "name": "<NAMED_ERROR>" }` — refusal with the recorded
  stable machine error name.

## Step kinds (the closed interpreter vocabulary)

| Kind | Operates on | Notes |
|---|---|---|
| `parse-manifest` | `flexcore_kernel::manifest::parse_manifest_value` | `index` into `inputs.manifests`; the parsed manifest is cached for later steps |
| `resolve` | `flexcore_kernel::resolve::resolve` | `manifests` (indices), optional `selections` (`[contract, plugin]` pairs), `as` names the plan |
| `parse-profile` | `flexcore_kernel::profile::parse_profile_value` | `index` into `inputs.profiles` |
| `validate-profile` | `flexcore_kernel::profile::validate_profile` | `profile` index, `manifests` indices, `as` names the plan |
| `diff-plans` | `flexcore_kernel::recomposition::diff_plans` | `current` / `target` plan names; counts pinned |
| `execute-switch` | `flexcore_kernel::recomposition::execute_switch` | `from`/`to` profile ids, plan names, optional `confirmation` (or `null`), optional `to-posture` (default `simulation-shadow`); steps run through the scripted in-test executor |
| `lifecycle-register` | `flexcore_kernel::lifecycle::PluginLifecycle::register` | fresh lifecycle at `Discovered` (replaces any previous one) |
| `lifecycle-gate` | the lifecycle gate methods | `transition` ∈ `verify` / `mark-compatible` / `configure` / `start` / `mark-ready`; `gate` ∈ `passed` / `failed` |
| `lifecycle-begin-drain` / `lifecycle-complete-drain` | the §5.6 drain machine | the drain port is the scripted in-test port (see `set-drain-script`) |
| `lifecycle-state` | `PluginLifecycle::state` | asserts the exact `LifecycleState` name (e.g. `READY`, `STOPPED`) |
| `lifecycle-record-version-change` | `PluginLifecycle::record_version_change` | `new-version` |
| `set-drain-script` | the scripted drain port | `quiesce` (bool), `outstanding`, `cancelled` — what the next drain reports |
| `resolve-config` | `flexcore_kernel::plugin_config::resolve_for_instance` | `manifest` index, `instantiated-version`, `config` index into `inputs.configs`, optional `as` names the resolved view |
| `build-envelope` | `plugin_path_protection::PluginRequestEnvelope::new` | correlation/plugin/sequence/window/payload fields; the interpreter ALWAYS builds twice and asserts byte-identical canonical string + digest (the P7.42 determinism pin on every scenario build); `as` names the envelope |
| `verify-envelope` | `PluginPathVerifier::verify_and_admit` | runs the four path checks against the scenario's verifier (created on first use over a scratch file checkpoint and the scenario's injected `FixedClock`); an `ok` ADVANCES the per-plugin sequence |
| `tamper-envelope` | — | clones a stored envelope and replaces its `payload` (public field), naming the copy `as` — the digest then mismatches |
| `advance-clock` | the injected `FixedClock` | `by` milliseconds |
| `record-outcome` | `PluginPathVerifier::record_outcome` | `response` echo and optional `proposal`; the `variant` pin maps to the closed `PluginPathOutcome` set |

## Suite version

`flexcore-plugin-sdk-scenario-regression-v1` — the id every CI run records
(P7.43 step 2). Changing any scenario file is a suite-version change: bump
the scenario's `scenario-version` and, when the step vocabulary or the set
of scenarios changes, the suite id in the CI step and the interpreter
constant together.

## Deliberate exclusions (recorded)

- **P7.38/P7.39 extension scenarios** (revocation, staged rollout, tenant
  scoping — e.g. the planned `sdk-05-revoke-then-verify-refused`) are NOT
  written: at authoring time the concurrent P7.38/P7.39 kernel/host files
  were not on disk, and the discipline forbids writing scenarios against a
  PLANNED API shape. They are recorded as a follow-up for the coordinator
  in `ledger/phase-07-plugin-sdk/41_42_43_hardening_determinism_regression.md`
  §6 — to be added once the revocation/tenant-scope APIs land and their
  decision record exists.
- **Class B/C execution** (WASM instantiation, gRPC worker calls) is NOT a
  scenario leg here: the kernel interpreter drives composition-side APIs
  only. The live host/worker execution legs belong to the P7.21/P7.22
  conformance and swap suites and to the P7.28 exit-gate run.
