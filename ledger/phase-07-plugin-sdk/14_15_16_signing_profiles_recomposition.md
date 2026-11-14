# P7.14 + P7.15 + P7.16 — Plugin signing/provenance/SBOM, composition profiles, and safe re-composition: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P7.14, P7.15, P7.16, Phase 7 ledger
(`ledger_phase_7.md`). Owner role: Rust engineer (SDK) (P7.16 per ledger;
P7.14/P7.15 executed by the same hand under the same crate ownership).
Depends on: P7.14 ← P7.05/P7.07 (manifest + lifecycle — satisfied, same
crate); P7.15 ← P7.06/P7.08 (resolver + registry — satisfied, same crate)
and P6.29 (`site-edge` boundary); P7.16 ← P7.07/P7.15 (satisfied
in-wave). Report trace: §9.3; §5.6; §5.7; §5.8; §5.4; §17.3–§17.4; §21.4;
§22.1 (ADR-005, ADR-010); §14.1.

**Run-status honesty (AGENTS.md rule 7; the loop bans carried over from
P7.05/P7.06 and P7.07/P7.08):** the code, data, and tests in this wave are
WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed (no build, no check, no test, no clippy, no fmt). Every test
below is UNRUN until the human build/test pass executes it. Nothing in
this document claims a test passed, a build succeeded, or a compile
completed. The only tools run were the three permitted verification
scripts (§9, output quoted verbatim).

## 1. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `crates/flexcore-kernel/src/signing.rs` | NEW (P7.14: provenance record + DigestVerifier port + the `Discovered → Verified` gate) |
| `crates/flexcore-kernel/src/profile.rs` | NEW (P7.15: `flexcore-profile-v1` loader/validator, mode lock, permission ceiling, profile→plan validation) |
| `crates/flexcore-kernel/src/recomposition.rs` | NEW (P7.16: explicit audited profile switching, plan diff, confirmation gate, execution ordering) |
| `crates/flexcore-kernel/tests/signing_p7_14.rs` | NEW (P7.14 tests) |
| `crates/flexcore-kernel/tests/profile_p7_15.rs` | NEW (P7.15 tests) |
| `crates/flexcore-kernel/tests/recomposition_p7_16.rs` | NEW (P7.16 tests) |
| `plugins/profiles/sim-lab.profile.json` | NEW (shipped profile data) |
| `plugins/profiles/vpp-operator.profile.json` | NEW (shipped profile data) |
| `plugins/profiles/site-edge.profile.json` | NEW (deferred shell — loads, composes nothing) |
| `plugins/profiles/research-gnn.profile.json` | NEW (shipped profile data) |
| `plugins/profiles/research-marl.profile.json` | NEW (shipped profile data) |
| `plugins/profiles/README.md` | NEW (the documented commented-out-entries pattern, ceiling model, mode lock) |
| `crates/flexcore-kernel/src/lib.rs` | EDITED (ADDITIVE ONLY: three module-table rows + three `pub mod` lines; no existing line changed) |
| `crates/flexcore-kernel/src/manifest.rs` | EDITED (ADDITIVE ONLY: `validate_plugin_id` made `pub` with a comment — the signing and profile modules re-use the plugin-id grammar for ids handed to them outside a manifest; logic unchanged, the exact P7.08 `validate_contract_id` precedent) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.14/P7.15/P7.16 ticks ONLY — P7.10/P7.11 belong to the concurrent agent and were not touched) |
| this file | NEW (decision record) |

Concurrency note honoured: `apps/plugin-host/**` and ledger ticks
P7.10/P7.11 belong to the other agent; none were read for editing
purposes or touched.

Dependency minimisation: no new dependency of any kind. The kernel crate
still takes exactly `serde_json` (workspace pin) — the digest layer is
dependency-free (the FNV-1a 64 test verifier is a 6-line local function,
the repo's existing P4.28/P6.14 pattern), and no crypto crate is
introduced (§2 below records why, and what the deployment must wire).

## 2. P7.14 — what was implemented (and the honesty record)

### 2.1 The digest-and-record layer (the P6.20 precedent applied)

The design follows `ledger/phase-06-edge-protocol/20_deployment_profile.md`
§6 exactly: the verification STRUCTURE is implemented; the production
signing identity is a DEPLOYMENT REQUIREMENT, recorded honestly and not
claimed. Concretely:

1. **The versioned provenance record** `flexcore-plugin-provenance-v1`
   (`crates/flexcore-kernel/src/signing.rs`): artifact-id, digest
   (lowercase hex) + digest-algorithm (closed vocabulary), signer key
   REFERENCE, signing time, manifest id + version (the record BINDS the
   manifest — a record naming a different manifest is refused:
   `MANIFEST_MISMATCH`), builder id (provenance metadata), and an SBOM
   reference (REQUIRED). Strict fail-closed parsing in the P7.05 style:
   closed key sets at the single level, unknown fields rejected, named
   errors, 64 KiB source cap.
2. **The pluggable `DigestVerifier` port**: `algorithm_name()` +
   `verify(artifact, declared_digest_hex) → Matches | Mismatch | Refused`.
   Three implementations:
   - [`NoVerifier`] — the DEFAULT, fail-closed: refuses EVERY artifact.
     A deployment that wires nothing verifies nothing, and nothing passes
     the gate. The absence of signing infrastructure can never look like
     success (the ledger's named risk — "signature verification that
     warns instead of rejecting" — is countered by construction).
   - [`UnsafeTestOnlyDigest`] — FNV-1a 64 over the artifact bytes, loudly
     labelled UNSAFE FOR SUPPLY-CHAIN INTEGRITY (a 64-bit
     non-cryptographic checksum; forged collisions are practical). It
     exists ONLY so the gate, tamper path, and audit trail are testable
     without a crypto dependency. The task's alternative — "a plain
     `fnv1a64` payload binding" as the supply-chain digest — is explicitly
     REJECTED as insufficient (as the task itself anticipated); it ships
     only as this labelled test double.
   - a production SHA-256 (or stronger) verifier — **DEPLOYMENT
     REQUIREMENT, NOT IMPLEMENTED**. No crypto dependency is added to the
     kernel crate; the composition root wires a real verifier through the
     port. This is the exact P6.20 PKI honesty pattern (§6/§7 of that
     record): the structure exists, the trust anchor does not, and no
     certification or security claim is made
     (`signing::SIGNING_STATEMENT` pins the boundary; a test asserts it).
3. **The `Discovered → Verified` gate**: `provenance_gate(...)` consumes
   the P7.07 types additively (`GateOutcome`, `AuditSink`, `Clock`,
   `LifecycleAuditRecord`) and returns the outcome the lifecycle's
   `verify()` consumes. Refusal order (fail-closed, pinned by tests): no
   record at all → `UNSIGNED_ARTIFACT` (rejected BEFORE any compatibility
   check — the P7.14 step-2 rule); record binds another manifest →
   `MANIFEST_MISMATCH`; record's algorithm ≠ configured verifier's →
   `DIGEST_ALGORITHM_MISMATCH`; digest mismatch → `DIGEST_MISMATCH`;
   verifier refusal → `VERIFIER_REFUSED`. A refused gate produces
   `GateOutcome::Failed`, so the lifecycle refuses the transition with
   `GATE_REFUSED` and the state is unchanged — a signature verification
   NEVER warns instead of rejecting.
4. **Audit on verify and refuse** (ledger test requirement): the gate
   emits a `LifecycleAuditRecord` through the in-module `AuditSink` port
   for EVERY outcome — `Transition` on acceptance, `Refused` on every
   refusal — with the attempted `Discovered → Verified` edge named and
   timestamps from the injected `Clock` only. The lifecycle's own commit
   adds the second record on acceptance.

### 2.2 Key-reference-only structure (structural field design)

The signer key appears as a REFERENCE only, scheme-prefixed
(`key-store:`, `env:`, `file:` — the P7.05 secret-reference scheme
pattern plus a key-store scheme for the deployment key store). A
reference without a scheme prefix is rejected as likely inline key
material (`PROVENANCE_KEY_REFERENCE_INVALID`), and the rejected VALUE is
never echoed back (pinned by test). The closed key set makes the record
STRUCTURALLY INCAPABLE of carrying key material: there is no field a key
could hide in — a field named `public-key` or `signing-key` is an
unknown-field rejection (pinned by test). No key material exists in code,
tests, or fixtures (the P4.28 secrets statement discipline).

### 2.3 SBOM: the seam, not the generation

Kernel-side the SBOM is metadata: the record's `sbom-reference` is
REQUIRED (an artifact without an SBOM reference is not provenanced, §9.3)
and rides the audit reason on acceptance. The SBOM GENERATION tooling is
P7.27's CI configuration (config-only, the P6.27 pattern — Syft per that
wave's tool decision); it is recorded here as the seam and deliberately
NOT implemented in this module. The P7.14 ledger risk — "SBOM generated
but never checked against the license audit (P7.03)" — is bounded by the
required reference: a release whose SBOM reference does not resolve is a
release-gate failure (P7.30/P7.27 verification), not a silent gap.

### 2.4 P7.38 design-for-later (skimmed, shaped, not built)

Revocation (P7.38) plugs in at three recorded seams: (a) the
`DigestVerifier` port gains a revocation-aware implementation (a record
whose key reference is revoked refuses exactly like a mismatch —
`VERIFIER_REFUSED`); (b) `signed-at-millis` and the key reference are
already on the record, so a revocation list can be keyed by (key
reference, signed-at window); (c) the P7.16 switch executor is the
enforcement point for "revoke a RUNNING plugin" (drain-and-remove). No
revocation list format, distribution, or staging decision is made here —
P7.38 owns them. Staged rollout (P7.38) rides the profile-switch record:
the audited before/after composition IS the stage boundary.

### 2.5 P7.14 acceptance mapping

| Ledger item | Where |
|---|---|
| Unsigned package rejected | `an_unsigned_artifact_is_refused_at_the_verified_gate_before_anything_else` (gate + lifecycle refusal + audit) |
| Tampered package rejected | `a_tampered_artifact_fails_the_digest_check_with_a_named_refusal` + `the_verifier_port_reports_the_mismatch_itself` (the test verifier "flips a bit" of the artifact bytes) |
| SBOM exists per package | the record's REQUIRED `sbom-reference` (metadata; generation is P7.27 CI — recorded, not implemented) |
| Rollback to the previous version works | artifact RETENTION is P7.38 supply-chain work + the P7.16 switch-back path (§4.5 below); not claimed here |
| Self-upgrade and runtime code-fetch fail | structural: the kernel has no fetch port and plugins have no capability surface reaching the artifact store (§5.7 never-granted includes `network.any`; the P7.11/P7.12 broker re-refuses); the ledger's hostile-package execution is P7.29/P7.13 testing, not this wave |
| Verification method (tamper + install) | the tamper tests run in this suite; the full install-tamper procedure is the human build/test pass (UNRUN) |

## 3. P7.15 — what was implemented

### 3.1 Profiles as DATA (`flexcore-profile-v1`)

`plugins/profiles/*.profile.json` — five versioned JSON documents, one
per §5.8 name (`sim-lab`, `vpp-operator`, `site-edge`, `research-gnn`,
`research-marl`), loaded by the strict validator in
`crates/flexcore-kernel/src/profile.rs`. Closed top-level key set
`{profile-schema, profile-id, name, version, description,
control-authority, permission-ceiling, selections}`; closed selection key
set `{manifest-id, version-constraint}`; unknown fields REJECTED (never
warned); named errors throughout (`ProfileError::name`, 22 distinct
names, all asserted pairwise distinct by test). A profile is data and
nothing else: there is no code path behind it and no hidden default —
the ledger's recorded risk ("a profile that is really a code path with
hidden defaults") is countered by construction, and the reviewable
surface is exactly the file.

**The commented-out-entries pattern (documented in
`plugins/profiles/README.md`):** a profile that cannot fully resolve in
v0.5.0 ships with its unresolvable selections COMMENTED OUT in the JSON.
The loader strips FULL-LINE `//` comments only (deterministic, before
parsing; `//` inside a string value is never affected; no trailing or
block comments). A commented selection is NOT loaded, NOT validated, and
NEVER composed; enabling one is an edit to the reviewed, audited file —
the ledger's "enabling a control-capable plugin requires an explicit,
reviewed profile change". Every commented block carries its reason
inline; the README §5.1 lists all of them. Per the ledger's step 4
("ship v0.5.0 with the profiles that are ready; record which profiles
are deferred and why"): `site-edge` ships as a DEFERRED SHELL — it loads
and is reviewable, and composes nothing (`PROFILE_EMPTY_SELECTIONS` at
plan validation, pinned by test); its README rows record why every entry
is deferred, and why the edge "limited command receiver" has NO entry at
all (it is protected kernel — P6.29 §3 — and cannot be expressed,
referenced, or smuggled by any profile).

**Seed-id ratification:** the active selections reference the DEFAULT-
labelled seed ids proposed in P7.08's `registry::seed`
(`builtin.simulator-telemetry`, `builtin.highs-optimizer`,
`builtin.pandapower-grid`) — this profile work RATIFIES them as the
v0.5.0 seed composition ids (the P7.08 record asked for exactly this).
They resolve when the corresponding manifests are supplied at
composition time; the profile tests supply fixture manifests with those
ids and assert the full profile→plan path.

### 3.2 The control-authority posture lock (§5.8/§21.4 made structural)

`control-authority` is a closed-vocabulary field and the profile
mechanism accepts EXACTLY one value: `simulation-shadow`. Any other
value — including the other §21.4 modes (`read-only-shadow`,
`operator-approved`, `bounded-automation`) — is the named error
`CONTROL_AUTHORITY_MODE_FORBIDDEN` (pinned by test for each). The
`ControlAuthorityPosture` enum carries the four §21.4 mode names: profile
DATA can only ever produce `SimulationShadow`; the other variants exist
so the P7.16 switch boundary can compare postures handed in from
non-profile code and refuse a mode change (defence in depth — §4.2).
Mode is a deployment/per-site setting; a profile, like a plugin, can
never select or raise it (§5.8/§21.4; 02_scope_freeze §4 verbatim).

### 3.3 The permission-ceiling model

`permission-ceiling` is the MAXIMUM set of §5.7 permission ids any
plugin in the profile may carry:

- entries from the allowed §5.7 vocabulary only
  (`PROFILE_PERMISSION_CEILING_UNKNOWN` otherwise);
- the never-granted set (`dispatch.authorize`, `dispatch.issue`,
  `secret.read`, `network.any`) cannot appear in a ceiling
  (`PROFILE_PERMISSION_CEILING_NEVER_GRANTED`) — a profile can never
  ceiling what no manifest may request;
- at plan validation every selected manifest's `permissions` must be a
  SUBSET of the ceiling (`PROFILE_PERMISSION_CEILING_EXCEEDED`) — the
  ceiling restricts, it never grants;
- the research profiles withhold even the proposal permissions: their
  ceiling is `telemetry.read` + `asset.read` only (DEFAULT decision D-2
  below; ADR-010/§13.4). The ledger's acceptance item "research-gnn /
  research-marl grant no dispatch authority (verified by attempting a
  dispatch from within them)" is enforced STRUCTURALLY here: no plugin
  in those compositions can hold a schedule- or dispatch-side
  permission, so there is no authority to dispatch with; the
  end-to-end dispatch attempt itself is P7.15's verification method
  (the ledger's own wording) and P7.31 handover — UNRUN here, with the
  structural half pinned by test
  (`a_research_profile_refuses_even_a_propose_permission_the_structural_no_dispatch_test`).

### 3.4 Profile → plan validation

`validate_profile(profile, manifests)` (deterministic order): every
selection resolves to a supplied manifest
(`PROFILE_SELECTION_UNRESOLVABLE`), satisfies its version constraint
(parsed range, never string-compared —
`PROFILE_MANIFEST_VERSION_UNSATISFIED`), stays under the ceiling; each
selection pins EVERY contract in its manifest's `provider-of` as an
EXPLICIT provider selection (`SELECTION_EXPLICIT` — a profile is the
reviewable place provider conflicts are resolved; two selections pinning
one contract is `PROFILE_DUPLICATE_CONTRACT_PIN`, never silent
last-wins); then the P7.06 resolver runs — the protected-kernel guard,
platform-API gate, missing-provider, cycle, and conflict checks are
wrapped, never weakened (`ProfileProtectedContractRefused` re-surfacing
`PROTECTED_KERNEL_REPLACE` by name; other resolver refusals wrapped as
`ProfileResolutionRefused` carrying the source error's name). Same
inputs → identical plan (pinned across repeated runs and reversed
manifest input order). A profile with empty selections loads (a shell)
but refuses composition (`PROFILE_EMPTY_SELECTIONS`).

### 3.5 P7.15 acceptance mapping

| Ledger item | Where |
|---|---|
| Each profile loads, resolves, starts its bundles | load: `each_shipped_profile_loads_from_its_definition_file_alone` (all five, from their files alone); resolve: the seed profiles resolve over fixture manifests (`profile_plan_validation_is_deterministic...`); "starts" = the composition root registering the plan (P7.08) — the lifecycle states are the P7.07 machine's, exercised at composition time; UNRUN here |
| A control-capable plugin without explicit declaration is refused | the ceiling model + the explicit-selection model: nothing enters a composition except through a declared, reviewed selection; enabling requires a profile-file edit |
| research profiles grant no dispatch authority | the ceiling (§3.3) + the structural test; the end-to-end dispatch attempt is P7.31/UNRUN |
| Silent-enable case refused | unknown fields/selections rejected (`profile_parse_rules_are_fail_closed`, including a smuggled `enable-dispatch` knob) |

### 3.6 Observed inconsistency recorded (not silently "fixed")

`plugins/manifests/builtin-ess-provider.manifest.json` declares
`"api-version": ">=0.5,<1.0"`, and the published schema document §2 uses
the same two-component form as an example — but the v1 validator's
version grammar (`manifest::SemVer::parse`) requires exactly three
components, so that manifest FAILS validation as written. This wave owns
neither file (both are P7.05 artifacts), so the inconsistency is
RECORDED, not patched: the `neryva.sim-builtin-ess` profile entry stays
commented out (README §5.1) until the example manifest (or the grammar,
or the schema-document example) is corrected by its owner. Flagged for
the human build/test pass.

## 4. P7.16 — what was implemented

### 4.1 Switching is an explicit, audited operation through the P7.07 lifecycle

`crates/flexcore-kernel/src/recomposition.rs` orchestrates: compute the
deterministic diff between the current and target `CompositionPlan`s
(P7.06 plans; versions compared too — a same-provider version bump is a
replacement, not a no-op), gate, execute, audit. Every step — committed
or refused — emits a `LifecycleAuditRecord` through the in-module
`AuditSink` port with injected-`Clock` timestamps; the audit trail is
`started → every applied step → committed` (or `→ refused`), and the
commit record carries the before/after composition evidence the ledger's
verification method asks for (the full diff is in the step records and
the returned `SwitchOutcome`).

### 4.2 The structural guarantees (in enforcement order)

1. `SWITCH_CONFIRMATION_MISSING` — no `SwitchConfirmation` (actor,
   from-profile, to-profile, timestamp, reason), no switch: fail-closed,
   nothing applied, refusal audited. **DEFAULT decision D-3: EVERY
   switch requires confirmation**, not only safety-relevant removals —
   stronger than the ledger minimum and simpler to audit; a switch is
   never silent. A confirmation naming different profiles than the
   request is `SWITCH_CONFIRMATION_MISMATCH`.
2. `CONTROL_AUTHORITY_MODE_CHANGE_REFUSED` — the request carries both
   postures; if they differ the switch is refused. With the P7.15 mode
   lock, profile data can only produce `simulation-shadow`, so this is
   defence in depth against postures handed in from non-profile code
   (§21.4: the mode is a deployment/per-site setting; a profile switch
   can never raise it).
3. `PROTECTED_CONTRACT_IN_TARGET` — the protected kernel is structurally
   unreachable, re-stated at the composition level: the P7.06 resolver
   refuses any protected-kernel claim BEFORE any plan exists
   (`PROTECTED_KERNEL_REPLACE`), so no resolver-produced plan can
   contain one; a plan containing one can only exist if hand-constructed
   past the resolver, and the switch refuses even that. Both layers are
   pinned by test (the composition-level guard over a hand-constructed
   plan + the resolver's structural refusal). A profile switch can never
   smuggle in a control-capable plugin.
4. `SWITCH_NO_OP_REFUSED` — a switch to an identical composition is
   refused, not silently echoed.
5. Drain-before-replace executes through the P7.07 semantics: an
   incumbent with outstanding work (a dispatch transaction active)
   cannot be drained, so a switch during active work is REFUSED — P7.07's
   recorded REFUSED-not-queued behaviour, re-pinned here at the
   composition level (`a_switch_with_a_dispatch_transaction_active_is_refused_not_queued`).

### 4.3 The executor port and its canonical wiring

`CompositionExecutor` is the seam the switch operates through (the
kernel's own port pattern — `DrainPort`, `AuditSink`, `Clock` are ports
for the same reason): `prepare_provider` / `drain_to_stopped` /
`replace_provider` / `remove_provider` / `add_provider`. The CANONICAL
composition-root implementation wraps the P7.08 `ProviderRegistry` and
P7.07 `PluginLifecycle`: `drain_to_stopped` runs
`begin_drain`/`complete_drain` (deadline expiry self-restores the
incumbent to Ready, fail-closed); `replace_provider` is the registry's
`swap_provider` (which independently re-checks
`require_fully_drained` — `REPLACEMENT_BLOCKED_UNDRAINED` — and records
the version change); `add_provider` registers through the fresh plan's
registration path (a populated registry is never re-planned in place —
`REGISTRY_ALREADY_POPULATED`; additions and the final plan binding are
composition-root work). The module doc pins this wiring; the tests use a
memory executor that models the canonical contract exactly (including
the self-restoring drain refusal and the drained-gate on replace) with
injected failures.

### 4.4 Execution ordering and partial-failure semantics (the recorded design)

```text
PHASE A  prepare (contract id order)     — eager construction of every
                                           newcomer/replacement provider.
                                           ZERO side effects: a failure
                                           aborts with the composition
                                           UNTOUCHED.
PHASE B  incumbents (plugin id order)    — for each departing/upgraded
                                           plugin: drain_to_stopped, and
                                           ONLY after the incumbent is
                                           confirmed stopped, its vacated
                                           contracts (replace / remove).
                                           A drain refusal is SELF-
                                           RESTORING (P7.07 restores the
                                           incumbent to Ready), so an
                                           abort here leaves the
                                           composition exactly as it was.
                                           Newcomers apply only after
                                           incumbents are confirmed —
                                           the recorded ordering rule.
PHASE C  newcomers (contract id order)   — add_provider, after ALL
                                           incumbent operations succeeded.
COMMIT                                   — audited; the recorded ACTIVE
                                           profile advances ONLY now.
```

**The honest limitation:** true atomicity across a multi-step composition
change is impossible without a process restart (the P7.08 registry is
per-composition and a populated registry is never mutated in place). The
ordering front-loads failure-prone work (phase A is side-effect-free;
phase-B drain failures self-restore), so the only residual window is a
failure DURING phase C — where the composition stands at the LAST
FULLY-APPLIED STATE (everything up to the failed step; nothing after).
The safe default is the RESTART FALLBACK: the composition root records
the active profile only after a fully successful switch, and on process
restart rebuilds the recorded active profile from its manifest set
(recovering drained incumbents through fresh lifecycles), then the
operator re-runs the switch. Every abort is audited (step refusal +
`ABORTED` record naming the recovery path). Pinned by two tests: a
drain failure (composition fully intact) and a failure at the last step
(everything before it applied, the newcomer not half-registered, the
abort record naming the restart fallback).

### 4.5 Rollback (ledger step 4)

Switching back is just another switch: the diff to the previous plan is
computed, confirmed, drained, applied, and audited with its own
before/after records (`execute_switch` is symmetric over plans). Artifact
retention for version rollback is P7.14/P7.38 supply-chain work
(§2.5), not claimed here.

### 4.6 P7.16 acceptance mapping

| Ledger item | Where |
|---|---|
| `sim-lab` → `vpp-operator` and back | the switch mechanism over resolver-produced plans, both directions symmetric (`a_clean_switch...`, `the_clean_switch_is_reproducible...`); the named-profile scenario runs at composition time (UNRUN here) |
| Switch while a dispatch transaction is active | refused (§4.2 item 5) |
| Switch to a profile with an incompatible plugin | the target plan is resolver-validated first (`PROFILE_RESOLUTION_REFUSED` upstream in P7.15; `PROTECTED_CONTRACT_IN_TARGET` here); incompatible providers never reach the executor |
| No switch changes the control-authority mode | §4.2 item 2 + the P7.15 mode lock (both layers pinned) |
| In-flight dispatch blocks or defers the switch | REFUSED (P7.07's recorded behaviour; §4.2 item 5) |
| Every switch audited with before/after composition | the started/step/committed record chain (`a_clean_switch...` asserts it) |

### 4.7 Named-error tables

**P7.14 (`SigningError::name`, 13):** `PROVENANCE_RECORD_JSON_INVALID`,
`PROVENANCE_RECORD_NOT_AN_OBJECT`, `PROVENANCE_SCHEMA_UNSUPPORTED`,
`PROVENANCE_UNKNOWN_FIELD`, `PROVENANCE_MISSING_FIELD`,
`PROVENANCE_WRONG_TYPE`, `PROVENANCE_MANIFEST_ID_MALFORMED`,
`PROVENANCE_MANIFEST_VERSION_INVALID`, `PROVENANCE_DIGEST_INVALID`,
`PROVENANCE_DIGEST_ALGORITHM_UNKNOWN`, `PROVENANCE_KEY_REFERENCE_INVALID`,
`PROVENANCE_SBOM_REFERENCE_INVALID`, `PROVENANCE_FIELD_INVALID`. Plus the
gate refusal tags: `UNSIGNED_ARTIFACT`, `MANIFEST_MISMATCH`,
`DIGEST_ALGORITHM_MISMATCH`, `DIGEST_MISMATCH`, `VERIFIER_REFUSED`.

**P7.15 (`ProfileError::name`, 22):** `PROFILE_JSON_INVALID`,
`PROFILE_NOT_AN_OBJECT`, `PROFILE_SCHEMA_UNSUPPORTED`,
`PROFILE_UNKNOWN_FIELD`, `PROFILE_MISSING_FIELD`, `PROFILE_WRONG_TYPE`,
`PROFILE_ID_MALFORMED`, `PROFILE_VERSION_CONSTRAINT_INVALID`,
`PROFILE_SELECTION_MANIFEST_ID_MALFORMED`, `PROFILE_DUPLICATE_SELECTION`,
`PROFILE_SELECTION_COUNT_EXCESSIVE`, `PROFILE_PERMISSION_CEILING_UNKNOWN`,
`PROFILE_PERMISSION_CEILING_NEVER_GRANTED`,
`PROFILE_PERMISSION_CEILING_DUPLICATE`, `CONTROL_AUTHORITY_MODE_FORBIDDEN`,
`PROFILE_EMPTY_SELECTIONS`, `PROFILE_SELECTION_UNRESOLVABLE`,
`PROFILE_MANIFEST_VERSION_UNSATISFIED`,
`PROFILE_PERMISSION_CEILING_EXCEEDED`, `PROFILE_PROTECTED_CONTRACT_REFUSED`,
`PROFILE_DUPLICATE_CONTRACT_PIN`, `PROFILE_RESOLUTION_REFUSED`.

**P7.16 (`RecompositionError::name`, 6):** `SWITCH_CONFIRMATION_MISSING`,
`SWITCH_CONFIRMATION_MISMATCH`, `CONTROL_AUTHORITY_MODE_CHANGE_REFUSED`,
`PROTECTED_CONTRACT_IN_TARGET`, `SWITCH_NO_OP_REFUSED`,
`SWITCH_STEP_REFUSED` (with the step named: `PREPARE`/`DRAIN`/
`REPLACE`/`REMOVE`/`ADD`). All three tables are asserted pairwise
distinct by their suites.

## 5. DEFAULT-labelled decisions (recommendations awaiting human sign-off)

| # | Decision | Label |
|---|---|---|
| D-1 | `NoVerifier` as the fail-closed default verifier; `UnsafeTestOnlyDigest` (FNV-1a 64) for tests only; production SHA-256 wiring = DEPLOYMENT REQUIREMENT (no crypto dependency added to the kernel crate) | DEFAULT — the P6.20 honesty pattern; a task-anticipated rejection of plain FNV as a supply-chain digest |
| D-2 | Research-profile ceilings withhold even proposal permissions (`telemetry.read` + `asset.read` only) | DEFAULT — stricter than the ledger minimum; ADR-010/§13.4; P7.31 handover may revisit under the proposal-only boundary |
| D-3 | EVERY profile switch requires a `SwitchConfirmation` (not only safety-relevant removals) | DEFAULT — stronger and simpler; fail-closed |
| D-4 | The provenance record's digest-algorithm vocabulary is the closed pair `{sha-256, fnv1a64-test}` — `sha-256` accepted at record level so deployment wiring needs no schema change, meaningless until a real verifier is configured | DEFAULT |
| D-5 | Profile loading strips FULL-LINE `//` comments only (the documented JSONC pattern for deferred selections); no trailing/block comments | DEFAULT |
| D-6 | Selections pin EVERY `provider-of` contract of the selected manifest (the profile IS the explicit selection); no separate per-contract pin field | DEFAULT — fewer fields, no silent conflict possible |
| D-7 | The switch executor is a port (`CompositionExecutor`) whose canonical implementation wraps `ProviderRegistry` + `PluginLifecycle` (the kernel's own port pattern; the kernel cannot depend on concrete provider types) | DEFAULT |
| D-8 | Switch execution ordering: prepare (side-effect-free) → per-plugin drain/replace/remove (plugin id order) → newcomers (contract id order) → commit; partial failure = last fully-applied state; restart fallback as the safe default | DEFAULT |
| D-9 | Switch-level audit records reuse the P7.07 `AuditSink` port and record shape (subject `composition.profile-switch`, version field = to-profile) | DEFAULT — one audit path per crate |
| D-10 | The seed built-in ids (`registry::seed`, DEFAULT in P7.08) are RATIFIED by the profile work as the v0.5.0 seed composition ids | DEFAULT |
| D-11 | Composition-size cap for profiles = the resolver's `MAX_COMPOSITION_MANIFESTS` (1024); profile/record source caps = 64 KiB | DEFAULT |
| D-12 | `site-edge` ships as a deferred shell (loads, composes nothing) per the ledger's "record which profiles are deferred and why" | DEFAULT |

## 6. Honest limitations (UNRUN and otherwise)

1. **UNRUN**: everything above is written, not built and not run — the
   hand-checked compile reasoning (borrow structure in the switch
   executor tests, the diff's closure-free rewrite, JSON fixture
   replacement strings) has not been checked by a compiler. The human
   build/test pass must run `cargo fmt`, `cargo clippy --workspace
   --all-targets -- -D warnings`, and `cargo test -p flexcore-kernel`
   and record results.
2. **No cryptographic signature exists anywhere.** The provenance record
   is the digest-and-record layer; a real signature scheme, key
   ceremony, trust anchors, and revocation are deployment/P7.38 work.
   `SIGNING_STATEMENT` pins this; no security claim is made (AGENTS.md
   rule 2).
3. **SBOM generation is not implemented** (P7.27 CI config, config-only);
   kernel-side it is the required `sbom-reference` metadata only.
4. **The profile→lifecycle "starts its bundles" acceptance** is
   composition-root work (register the plan, walk the P7.07 machine) —
   this wave delivers the data, the validation, and the switch
   orchestration, not a running composition.
5. **serde_json duplicate-key collapse** applies to profile and
   provenance parsing too (the P7.05 recorded limitation; P7.41 material).
6. **The `builtin-ess-provider` example manifest inconsistency** (§3.6)
   is recorded, not fixed — neither file is this wave's.
7. **No fuzz/property tests yet** (P7.41); the parsers are total (every
   input → Ok or a named error), bounded, and free of `unwrap`/`expect`
   on untrusted input.

## 7. Related records

- `05_06_manifest_resolution.md` — the manifest/resolver types consumed
  here (`Manifest`, `CompositionPlan`, `ExplicitSelections`,
  `PROTECTED_KERNEL_CONTRACT_IDS`, `CURRENT_PLATFORM_API_VERSION`, the
  id grammars).
- `07_08_lifecycle_registry.md` — the P7.07 gate/lifecycle surface
  P7.14 plugs into (`GATE_PROVENANCE` was left for exactly this wave);
  the seed ids P7.15 ratifies (D-10); the swap gate P7.16's canonical
  executor wraps.
- `02_scope_freeze.md` — the never-granted set, the §4 control-authority
  statements, the deferred categories the profile placeholders cite.
- `04_seam_inventory.md` — the protected non-seams the profile cannot
  reference; the edge non-seams behind the `site-edge` deferrals.
- `ledger/phase-06-edge-protocol/20_deployment_profile.md` — the
  signed-update design-only precedent P7.14 follows (§2).

## 8. Verification tools (permitted only; output verbatim)

See §9 below for the verbatim output. No cargo command was run.

## 9. Status

IMPLEMENTED (written), NOT BUILT, tests NOT RUN — UNRUN until the human
build/test pass. No `todo!()`/`unimplemented!()`/TODO markers; no
`unwrap`/`expect` on untrusted input in `src/` (test files may unwrap).
