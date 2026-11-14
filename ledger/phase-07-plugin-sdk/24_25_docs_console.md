# P7.24/P7.25 — SDK documentation and examples; the Plugins & Integrations console workspace

Date: 2026-11-13. Subtasks: P7.24, P7.25 (`ledger_phase_7.md` §P7.24, §P7.25).
Owner roles: technical writer + SDK engineer (P7.24); frontend engineer
(P7.25). Report trace: §8.1, §22.2 (P7.24); §20.1 item 10, §20.2, §5.6,
§5.8 (P7.25). Inputs: P7.05–P7.20 (the built kernel/host surfaces these
documents describe), the Phase 5 console.

## 0. Honest status — READ FIRST

**Everything in this wave is WRITTEN-UNRUN.** No cargo command, no npm
command, no test execution, no build (the loop bans). The console tests
(`PluginsWorkspace.test.tsx`) and the control-plane tests
(`tests/api_plugins.rs` + the in-module unit tests in `routes_plugins.rs`)
are WRITTEN-UNRUN; the SDK documents describe the BUILT system but no
claim in them is a tool-verified fact. The ledger ticks read
"implemented, awaiting human build/test" — never "tested".

The ledger's P7.24 step 3 asks for the bad example's "recorded rejection
output". **It cannot be recorded yet and is not pretended** (see §3.2):
the exact named error each violation produces is recorded from the
validator source instead, and the PREPARED-UNRUN commands that will
produce the actual output are listed verbatim.

## 1. P7.24 — the SDK documentation inventory

All NEW under `docs/plugin-sdk/`:

| Document | Content |
|---|---|
| `overview.md` | What the SDK is (ADR-005 capability seams, the three classes, the six protected kernel contract ids — each with its report citation as recorded in `resolve.rs`), the CAN/CANNOT tables (the enforcement story told plainly: the dispatch-bypassing plugin is UNBUILDABLE), the v0.5.0 simulation/shadow boundary, reading order |
| `manifest-guide.md` | The condensed field table, a valid annotated example, the §5.7 permission vocabulary, the config-schema subset, the secret-reference rule, version negotiation, the common-error table, the PREPARED-UNRUN test command |
| `class-a-builtin.md` | Registration path (registry/CompositionPlan), the seed built-ins, the Class A rules (manifest gate, protected ids, ceiling, lifecycle) |
| `class-b-wasm.md` | The WIT worlds + mapping tables (no contract invented), the HONEST UNVERIFIED binding status (`BINDINGS_NOT_GENERATED`, fail-closed `NoBindings`), the exact PREPARED-UNRUN commands from `wit/TOOLCHAIN.md`, the fail-closed door table, the candidate-only rule |
| `class-c-worker.md` | The worker contract, the template usage, the five isolation rules, the PREPARED-UNRUN grpcio/buf commands from `python/workers/README.md` |
| `lifecycle-and-profiles.md` | The §5.6 states + machine names, the frozen transition table + crash edges, the five gates, drain-before-replace with the deadline refusal, EXPLICIT-only restart, the five profiles, the mode lock, P7.16 switching with the named confirmations |
| `configuration-and-secrets.md` | The config rules with every named error, the defaults-recorded rule, the constraints document, the reference→handle seam |
| `examples/bad-plugin/` | `bad-manifest.json` (a real document the real validator rejects), `bad-plugin-source.py` (annotated pseudo-source — explicitly NOT runnable), `README.md` (the error-catalogue walkthrough, §3.2 below) |
| `examples/echo-worker/README.md` | The good minimal example as a REFERENCE to `python/workers/worker_template/example_echo_worker.py` (never duplicated) |

Honesty rules carried: every never-run step is labelled PREPARED-UNRUN
with its exact command and its pin source (`wit/TOOLCHAIN.md`,
`python/workers/README.md`); US "license" throughout; tables over prose;
every §N reference resolves to a real report heading; no certification or
safety claim anywhere; every error name, type name, field name, and
constant was verified against the source file named inline.

### 1.1 DEFAULT-labelled decisions (P7.24)

| Decision | Status |
|---|---|
| The SDK guide ships as `docs/plugin-sdk/` (the ledger's §8.1 trace) while the working examples stay REFERENCE documents pointing at the shipped sources (`plugins/manifests/*.manifest.json`, `python/workers/worker_template/example_echo_worker.py`) instead of duplicating them — a duplicated example would drift from the code of record | DEFAULT |
| The bad example's manifest carries ALL the ledger-required violations in one document, and the README walks them gate-by-gate (fix-one-see-the-next), because the validator aborts at the first violation — a single-document catalogue walkthrough covers more of the surface than one violation per file | DEFAULT |
| The WASM component example stays the P7.09 DESIGN note (`plugins/wasm-examples/README.md`): its materialisation is owned by the P7.10 host-prototype wave; this wave does not invent component source code | recorded in `plugins/wasm-examples/README.md` §5, carried |

## 2. P7.25 — the Plugins & Integrations workspace

### 2.1 The control-plane read seam (additive only)

| File | Status | Content |
|---|---|---|
| `apps/control-plane/src/dto_plugins.rs` | NEW | `PluginInventoryDto` / `PluginRecordDto` / `PluginHealthDto` / `ConfigPropertyDto` / `ProfileRecordDto` — the P6.25 honesty contract: absent runtime facts are explicit gaps (`lifecycle_state: NOT_REPORTED`, `health: None`), never plausible values |
| `apps/control-plane/src/routes_plugins.rs` | NEW | `GET /api/v1/plugins`; the `PluginInventorySource` seam + shipped default `RecordedShippedInventory` (exactly the P6.25 `EdgeStatusSource` pattern); in-module unit tests |
| `apps/control-plane/src/{lib.rs,state.rs,openapi.rs,routes.rs}` | EDIT (additive) | module declarations; the `plugins_inventory` seam field + `with_plugins_inventory` builder + the recorded shipped default in `AppState::new`; the OpenAPI path + schemas; the ONE-line additive router merge in `stage1_router` (the recorded P5.31/P6.06 merge point — this single line is outside the wave's stated file list and is recorded here as the minimal correct wiring) |
| `apps/control-plane/tests/api_plugins.rs` | NEW | the contract tests (written, UNRUN) |

**DATA SOURCE (the honest seam):** the control plane holds NO running
plugin host — the kernel is a library, not a service. The shipped default
serves the RECORDED inventory of the shipped manifest/profile files
(recorded verbatim 2026-11-13: the three `plugins/manifests/*.json` ids
with their metadata, contracts, permissions, config schemas, and the five
`plugins/profiles/*.json` compositions), with every RUNTIME fact (lifecycle
state, health) an explicit gap. A deployment composes its host/health
forwarder via `with_plugins_inventory`. The three seed built-in ids the
profiles select (`builtin.simulator-telemetry`, `builtin.highs-optimizer`,
`builtin.pandapower-grid`) have no shipped manifest files and are therefore
NOT plugin records — the surface note says so verbatim.

**The tenant-scoping statement:** the inventory is the deployment's
recorded plugin set — plugin composition is per deployment (report §5.8),
not tenant-partitioned business data. The caller's tenant still comes from
the verified claims for the authorization check (§16.2; no request-carried
tenant anywhere); the honest scoping statement is in the verbatim surface
note.

**READ-ONLY:** no activate/deactivate/upgrade/profile-switch endpoint
exists on this surface and none was invented. Composition changes take
effect ONLY through the reviewed profile-file edit + re-composition
procedure (P7.16).

### 2.2 Authz: the recorded-drift note

The ledger's action id for this surface is `plugin.inventory.read`, but
the `Action` vocabulary is a CLOSED enum owned by `crates/flexcore-authz`
(outside this wave's file ownership). The route therefore evaluates
`telemetry.read` — already tenant-scoped and granted to ALL FOUR roles by
the phase policy sets — as the interim deny-by-default gate, exactly the
`authorize_live_stream` (P5.25) and `get_edge_status` (P6.25) precedent.
The single swap point is the `authorize_tenant_scoped` call site in
`get_plugin_inventory`. When `Action::PluginInventoryRead` is added (one
variant + its `id`/`parse` arms + the role grants, the recorded
closed-enum extension rule), this call site is the whole change. Deny-by-
default holds either way: an empty policy set is 403 (pinned in
`api_plugins.rs`).

### 2.3 The console extension

| File | Status | Content |
|---|---|---|
| `console/src/api/plugins.ts` | NEW | the DTO mirrors (wire names), the closed vocabularies (lifecycle §5.6, health P7.20, readiness), the GET-only fetch wrapper, the display-only guarantee header |
| `console/src/workspaces/pluginsWorkspaceData.ts` | NEW | the loader + pure mapping (verbatim states/gaps), the local profile-switch request log (the denialLog pattern), the verbatim notes |
| `console/src/workspaces/PluginsWorkspace.tsx` | NEW | the extension section: inventory table, config summary (schema only), publisher gap, lifecycle/health verbatim-or-gap, profile composition table with the mode lock, the composition-change gap, the REQUEST-only switch affordance, the audit-trail gap, the proposed-vs-executed statement, the freshness line |
| `console/src/workspaces/PluginsWorkspace.test.tsx` | NEW | the vitest tests (written, UNRUN) |
| `console/src/App.tsx` | EDIT (additive) | one import + one wrapper component (`PluginsIntegrationsWithInventory`) composing the P5.07 workspace with the new section on the same `plugins-integrations` route; `PluginsIntegrations.tsx`, `pluginsData.ts`, and `registry.ts` are NOT modified |

**The P5/P7 coexistence note:** the P5.07 workspace above the new section
still states "no plugin kernel exists" — its honest PHASE 5 record. The
new section's banner explicitly scopes itself as the Phase 7 addition
(the SDK exists as WRITTEN-NOT-BUILT code) and never edits the P5
statement: the two statements are time-scoped, and both stay verbatim.

**The publisher display:** manifest schema v1 carries no publisher field
(deferred to P7.14 provenance, per `manifest-schema-v1.md` §8); the panel
renders that as the named gap `NOT RECORDED`, never a guessed publisher
(the ledger's "plugin list with publisher" is met honestly).

**The config summary:** property NAMES + TYPES + descriptions only; the
surface serves no configuration VALUE at all (§5.7; no secret value or
secret location beyond the recorded manifest reference strings is
anywhere on the endpoint).

### 2.4 The console display-only guarantees (recorded)

1. **No mutation endpoint called or invented:** the client module issues
   GET calls only; the server has no plugin mutation path (verified
   against `routes_plugins.rs` / `openapi.rs`; pinned by the OpenAPI
   POST-probe test in `api_plugins.rs`).
2. **The profile-switch affordance records a REQUEST only** (the P6.25
   command-path rule): local console event, the denialLog pattern, with
   the verbatim note that the change takes effect ONLY through P7.16's
   reviewed profile-file edit + re-composition procedure.
3. **Explicit confirmation required** before the request is recorded:
   the ConfirmHighImpact PATTERN (required checkbox in a `<label>`,
   aria-required, keyboard-operable) implemented in-place, for the same
   recorded reason as P6.25 — the recorded `HIGH_IMPACT_ACTIONS` list in
   `components/ConfirmHighImpact.tsx` is a closed Phase-5 vocabulary this
   wave does not own, so passing an unlisted actionId would render its
   unknown-action alert.
4. **No proposed-as-executed:** the `PROPOSED_VS_EXECUTED_NOTE` renders on
   the panel; plugin outputs are candidates (ADR-010); a recorded request
   is a REQUEST.
5. **Closed vocabularies verbatim:** lifecycle §5.6 machine names and the
   P7.20 health/readiness vocabulary render as the server sent them —
   never prettified into healthy-looking synonyms.
6. **Gaps named:** `NOT_REPORTED` lifecycle, `null` health, publisher,
   composition-change record, and the audit trail all render as explicit,
   explained gaps.

### 2.5 DEFAULT-labelled decisions (P7.25)

| Decision | Status |
|---|---|
| Interim authz gate `telemetry.read` instead of a new closed-enum action (§2.2) | recorded drift, the P5.25/P6.25 precedent |
| The inventory default serves the RECORDED shipped files with runtime facts as gaps (§2.1) | DEFAULT — the honest seam; no host reporter is composed in 0.x |
| The switch affordance exists as a REQUEST recorder (not suppressed entirely) because ledger P7.25 step 3 demands the confirmation flow; it records locally and names the P7.16 procedure | DEFAULT, mirroring the P6.25 rule |
| The new section renders below the P5.07 workspace on the same route via App.tsx composition, leaving the P5 file untouched | DEFAULT (the SitesEdgeStatus new-files-over-rewrites precedent) |
| `ApiError::infrastructure` is documented as 500 (its actual status) in the P7.25 OpenAPI annotation — the P6.25 annotation says 503 while the code returns 500; this wave documents the real status and does not touch the P6 file | DEFAULT |

## 3. The bad-example error catalogue (each name verified against source)

### 3.1 The catalogue

| # | Violation | Gate | Named error | Source verified |
|---|---|---|---|---|
| 1 | requests `dispatch.authorize` (d) | manifest validator | `PERMISSION_NEVER_GRANTED` | `crates/flexcore-kernel/src/manifest.rs` |
| 2 | scheme-less secret reference (b) | manifest validator | `SECRET_REFERENCE_INVALID` | `crates/flexcore-kernel/src/manifest.rs` |
| 3 | `resources: {}` — bounds omitted (f) | manifest validator | `MISSING_FIELD` | `crates/flexcore-kernel/src/manifest.rs` |
| 4 | `forecast.publish` above the `sim-lab` ceiling (c) | profile plan validation | `PROFILE_PERMISSION_CEILING_EXCEEDED` | `crates/flexcore-kernel/src/profile.rs` |
| 5 | claims `kernel.dispatch-validation` (a) | resolver | `PROTECTED_KERNEL_REPLACE` | `crates/flexcore-kernel/src/resolve.rs` |
| 6 | `api-version: "^0.4.0"` does not cover platform `0.5.0` (e) | resolver | `PLATFORM_API_UNSUPPORTED` | `crates/flexcore-kernel/src/resolve.rs` |
| 7 | `"api-key": "<cleartext>"` in config (b) | config resolution (`Compatible → Configured`) | `SECRET_VALUE_IN_CONFIG` | `crates/flexcore-kernel/src/plugin_config.rs` |
| 8 | never-granted id at the host door (d, later layer) | broker + enforcement | `PERMISSION_NEVER_GRANTABLE` | `apps/plugin-host/src/{broker,enforcement}.rs` |
| 9 | over-broad request at the host door (c, later layer) | enforcement gate | `PERMISSION_ABOVE_PROFILE` | `apps/plugin-host/src/enforcement.rs` |

Each of names 1–7 is also pinned as a written-unrun test assertion in the
kernel suites; the host-door names in the host suites. Six independent
gates must simultaneously regress for the bad example to pass — the
ledger's "the bad example accidentally passing" risk is countered
structurally.

### 3.2 The recorded rejection output — HUMAN GATE

The output cannot be recorded in this loop (nothing runs). The verbatim
commands that produce it, to be recorded next to their tool outputs:

```sh
# Manifest-gate walkthrough (kernel suite):
cargo test -p flexcore-kernel --test manifest_p7_05
# Resolver / profile / config gates:
cargo test -p flexcore-kernel
# Host door (broker + enforcement):
cargo test -p flexcore-plugin-host
```

## 4. Per-subtask ledger coverage

| Subtask step | Delivered | Where |
|---|---|---|
| P7.24 step 1 (the SDK guide) | the seven documents of §1 | `docs/plugin-sdk/` |
| P7.24 step 2 (working examples) | the echo-worker reference guide + the shipped manifests as the annotated examples; the WASM component stays the P7.10-owned design note | `docs/plugin-sdk/examples/echo-worker/`, `plugins/manifests/` |
| P7.24 step 3 (the deliberately bad example) | the manifest + pseudo-source + catalogue walkthrough (§3) | `docs/plugin-sdk/examples/bad-plugin/` |
| P7.24 step 4 (verify_docs after doc changes) | run at wave end (actual output quoted in the ledger tick report) | — |
| P7.25 step 1 (the plugin list with capabilities/permissions/versions/health/config) | the inventory panel (§2.3) | `PluginsWorkspace.tsx` |
| P7.25 step 2 (profile composition + last change with actor and audit ref) | the composition table + the NAMED gap for the change record (§2.3) | `PluginsWorkspace.tsx` |
| P7.25 step 3 (explicit confirmation for high-impact actions) | the REQUEST-only affordance with required confirmation (§2.4.3) | `PluginsWorkspace.tsx` |
| P7.25 step 4 (proposed vs executed distinction) | the panel-level statement + the REQUEST/PROPOSAL wording (§2.4.4) | `PluginsWorkspace.tsx` |

Deliberately NOT delivered (recorded): no working WASM component source
(P7.10's ownership); no plugin mutation endpoint anywhere (the composition
change path is P7.16's, OFF the console); no audit-trail fabrication (the
gap is named).

## 5. Verification status

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN at wave end — actual output quoted in the completion report (the tool checks the report in `docs/plan/`; the new `docs/plugin-sdk/` tree is outside its scope and was verified not to break it) |
| `python tools/validate_agent.py` | RUN at wave end — quoted |
| `python -I tools/check_dependency_rule.py` | RUN at wave end — quoted (the kernel's dependency rule is untouched: `routes_plugins.rs` depends only on `flexcore-authz`/`flexcore-domain` types the control plane already depends on) |
| Console tests (`PluginsWorkspace.test.tsx`) | WRITTEN, NEVER RUN |
| Control-plane tests (`api_plugins.rs` + in-module unit tests) | WRITTEN, NEVER RUN |
| The PREPARED-UNRUN toolchain commands quoted in the docs | NEVER RUN (the loop bans; `wit/TOOLCHAIN.md` / `python/workers/README.md` own the procedures) |
