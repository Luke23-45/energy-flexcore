# P7.12/P7.13 — Permission enforcement and the sandbox negative suite

Date: 2026-11-13. Subtasks: P7.12, P7.13 (`ledger_phase_7.md` §P7.12, §P7.13).
Owner role: Security engineer. Report trace: §5.4 Class B, §5.7, §9.1, §13.4,
§17.4, §17.5, §19.2(7), §7.3 ([R4] Wasmtime security). Inputs: P7.10/P7.11
(`10_11_plugin_host_broker.md`, the host crate and the capability broker);
P7.05/P7.06 (`05_06_manifest_resolution.md`, the manifest types); P7.07
(`07_08_lifecycle_registry.md`, drain-before-replace — cited, kernel-owned);
P7.40 (`40_toolchain_pinning.md`, the wasmtime pin and the R7-C2 CVE floor);
`04_seam_inventory.md` §6 and `02_scope_freeze.md` (the protected-kernel
non-seams, incl. the ADR-010 P-4 invariant).

## 0. Honest status — READ FIRST

**Everything in this wave is WRITTEN-UNRUN.** No cargo command was executed
(no install, no build, no test): every test in
`apps/plugin-host/tests/{enforcement_p7_12,sandbox_p7_13}.rs` is
WRITTEN-UNRUN, and the first human build (`cargo test -p flexcore-plugin-host`)
is the verification gate. No claim in this record is a tool-verified fact; all
statements about behaviour are design statements over the code as written.
Consequently the tick lines in the ledger say "implemented, awaiting human
build/test" — never "tested".

Where a claim CANNOT be made from the host API at all (no component artifact
exists; the default build's bindings seam is fail-closed `NoBindings`), the
claim is recorded as a **HUMAN GATE** item with the exact test to run
post-first-build (§7). No sandbox-escape coverage is claimed beyond what the
seams can provide.

## 1. What was built (per-subtask coverage)

| Subtask | Delivered | Where |
|---|---|---|
| P7.12 core (permissions enforced, not decorative) | The enforcement gate: a component is instantiated ONLY with the capability set derived from (manifest permissions ∩ profile ceiling) filtered through the broker's scope narrowing; the gate is the ONLY constructor path to a binding | `apps/plugin-host/src/enforcement.rs` (`EnforcementGate::evaluate`, `instantiate`) |
| P7.12 fail-closed door | Unknown id → `PERMISSION_UNKNOWN`; never-granted id → `PERMISSION_NEVER_GRANTABLE`; over-broad (declared above the ceiling) → `PERMISSION_ABOVE_PROFILE`; duplicate declaration → `PERMISSION_DUPLICATE_DECLARATION`; broker scope/expiry refusals surface under their broker names — ALL before any component code runs | `enforcement.rs` (evaluate pipeline steps 3–7) |
| P7.12 drift cross-check (the recorded seam) | Any unalignable manifest field — the ledger's `timeoutMs` spelling vs the kernel's `timeout-ms`; any fuel/concurrency field the kernel `ResourceBounds` does not carry — is `PERMISSION_VOCABULARY_DRIFT`, never a default interpretation | `enforcement.rs` (`RESOURCE_FIELD_VOCABULARY`, evaluate step 2) |
| P7.12 frozen capability set | The verdict owns cloned grants, frozen at instantiation; a mutated manifest cannot expand an issued verdict; per-call broker re-checks can only tighten; re-instantiation via drain-before-replace is the only path (cited, not implemented) | `enforcement.rs` (`EnforcementVerdict`, the freeze audit record) |
| P7.12 candidate-only rule (§13.4) | `schedule.propose`/`dispatch.propose` grants make candidates only; no authorization id exists in any vocabulary; the broker forward returns `()` (type-level, re-asserted lexically) | `enforcement_p7_12.rs` (`a_proposed_schedule_gains_no_authority_from_being_proposed`, `the_validation_forward_returns_no_verdict_type_level`) |
| P7.12 audit | Every allow (with the exact granted scope), every refusal (with the named reason), and the freeze record is audited; audit-or-refuse in BOTH directions; an unaudited allow rolls its grants back (revocation) | `enforcement.rs` (`EnforcementAuditSink`, `rollback_grants`); `enforcement_p7_12.rs` |
| P7.12 no-raw-path | `host.rs::bind_component` demoted `pub` → `pub(crate)` (VISIBILITY-ONLY change); `enforcement::instantiate(&verdict, &artifact)` is the only public path; the verdict is unforgeable (private fields, no public constructor) | `host.rs`, `enforcement.rs`; lexical assertions in `enforcement_p7_12.rs` (`no_raw_instantiation_path_exists`) |
| P7.12 §5.7 matrix | Filled for Class B with code locations (§2 below); Class A and Class C cells point at their owning seams (recorded, not implemented here) | §2 of this record |
| P7.13 (a) ambient access | Path traversal/`..`/absolute/sibling-prefix/symlink-style reads refused (`SCOPE_OUTSIDE_GRANT`); network, env, clock, subprocess ids refused (`PERMISSION_NEVER_GRANTABLE`/`PERMISSION_UNKNOWN`); lexical zero-`wasi:` world proof | `sandbox_p7_13.rs` (a) tests |
| P7.13 (b) exhaustion | The five named limit errors carry the teardown contract at the type level; fuel is mandatory at the door (no unmetered instance); defaults are caps; declared bounds re-validated | `sandbox_p7_13.rs` (b) tests; `limits.rs` unchanged |
| P7.13 (c) API escapes | Ungranted/revoked/expired capability use refused with named errors; grant forgery structurally refused (the broker's grant type is broker-constructed since this wave — fields `pub` → `pub(crate)`, VISIBILITY-ONLY change + additive getters) and behaviourally futile (a captured grant object is inert after revocation) | `broker.rs` (visibility only); `sandbox_p7_13.rs` (c) tests |
| P7.13 (d) dispatch bypass (ADR-010) | A manifest requesting dispatch authority or any protected-kernel capability is refused at the gate AND the broker; the exhaustive sweep proves no permission id conveys dispatch authority — UNBUILDABLE, not discouraged | `sandbox_p7_13.rs` (d) tests; §8 below |
| P7.13 (e) resource-bound lie | The host enforces the DECLARED bounds (mirrored + re-validated); a constructed `ResourceLimits` cannot be widened (no public constructor other than `from_declared_bounds`); the verdict carries the re-validated limits | `sandbox_p7_13.rs` (e) tests |

## 2. The §5.7 enforcement matrix (P7.12 step 1 — Class B filled; A/C are seams)

Enforcing code locations are absolute in this crate. Class A (built-in Rust)
is enforced at the kernel provider registry (P7.08, `crates/flexcore-kernel/`
— another wave's ownership, untouched here); Class C (gRPC worker) is
enforced in the worker template and its supervision (P7.17/P7.18 seams — not
built in this loop; recorded honestly as the ledger's own risk "enforcement
in the WASM host but not in the gRPC worker path").

| §5.7 row | Class A (builtin-rust) | Class B (wasm-component — THIS crate) | Class C (grpc-worker) |
|---|---|---|---|
| `telemetry.read` | Kernel registry (P7.08 seam) | ALLOWED, scoped — `enforcement.rs::evaluate` step 5–7 + `broker.rs::derive_grants`; per-call `broker.rs::check` | P7.17/P7.20 seam |
| `asset.read` | Kernel registry (P7.08 seam) | ALLOWED, scoped — same cells as above | P7.17/P7.20 seam |
| `forecast.publish` | Kernel registry (P7.08 seam) | ALLOWED, scoped (send-only target) — same cells | P7.17/P7.20 seam |
| `schedule.propose` | Kernel registry (P7.08 seam) | CANDIDATE-ONLY — grantable, send-only; the proposal enters the protected path with no authority (`enforcement.rs` pipeline + the candidate-only tests) | P7.17/P7.20 seam |
| `dispatch.propose` | Kernel registry (P7.08 seam) | CANDIDATE-ONLY — same cells as `schedule.propose` | P7.17/P7.20 seam |
| `dispatch.validate.request` | Kernel registry (P7.08 seam) | CANDIDATE-ONLY, fenced: send-only to exactly `protected-validator` (`broker.rs::validate_request_target`); the forward returns `()` (`broker.rs::request_validation_forward`) | P7.17/P7.20 seam |
| `dispatch.authorize` | NEVER (kernel) | NEVER — `enforcement.rs::evaluate` step 4 (`PERMISSION_NEVER_GRANTABLE`) AND `broker.rs::grant`/`check` re-refusal | NEVER (P7.17 template must re-refuse; seam) |
| `dispatch.issue` | NEVER (kernel) | NEVER — same cells as `dispatch.authorize` | NEVER (seam) |
| `secret.read` | NEVER (kernel) | NEVER — same cells | NEVER (seam) |
| `network.any` | NEVER (kernel) | NEVER — same cells | NEVER (seam) |

Tenant-scope enforcement (P7.12 step 4) is the `DataScope` narrowing at grant
time (`broker.rs::CapabilityScope::covers` — tenant → site → asset, no
widening) applied through the gate; a site-A-scoped grant does not cover a
site-B request (`SCOPE_OUTSIDE_GRANT`, audited). The database-table mutation
escalation of ledger step 3 has no host-side surface: a plugin holds no
database capability id at all (`PERMISSION_UNKNOWN`) and the repositories are
protected non-seams (`04_seam_inventory.md` §6.1, ADR-007) — a plugin cannot
address them, which is the structural form of that refusal.

## 3. The enforced-at-the-door model

The gate's pipeline (fail-closed order, `enforcement.rs::evaluate`):

1. identity non-empty;
2. vocabulary alignment (`PERMISSION_VOCABULARY_DRIFT` on any unalignable
   resource-field name, duplicates included);
3. ceiling soundness;
4. declared permissions re-checked against the closed vocabulary;
5. intersection with the ceiling — over-broad REFUSES (`PERMISSION_ABOVE_PROFILE`), never trims;
6. declared resource bounds re-validated (`LIMITS_INVALID`) — validated
   BEFORE the derivation so no grant can outlive a refused request;
7. broker scope narrowing (`derive_grants`; broker refusals surface under
   their broker names, atomic batch);
8. allow-audit + freeze-audit; if either write fails the just-issued grants
   are REVOKED and `AUDIT_UNAVAILABLE` is returned — unaudited authority is
   not authority, and a revocation stands even if its own audit write fails
   (the broker's recorded asymmetry).

There is no API that instantiates from raw manifests: `bind_component` is
`pub(crate)` (visibility-only demotion), and `instantiate` requires an
`EnforcementVerdict`, obtainable only from `evaluate`. The lexical assertions
in `enforcement_p7_12.rs::no_raw_instantiation_path_exists` pin this API
shape at the source level.

## 4. The frozen-capability rule

The enforced capability set is FROZEN AT INSTANTIATION:

- The verdict owns CLONED grants; no `&mut` path exists; `Clone` copies
  freeze further. A mutated manifest object re-evaluated produces a NEW
  verdict; the issued one is untouched (tested with `PartialEq`).
- The broker's per-call re-checks govern every USE, so revocation and expiry
  can only TIGHTEN a running instance (tested: revoked → `CAPABILITY_NOT_GRANTED`
  while the frozen set still records what was granted at the door).
- The only path to a DIFFERENT capability set is re-instantiation through
  drain-before-replace — `crates/flexcore-kernel/src/lifecycle.rs`
  (`is_legal_transition`, `DrainPort`, `DrainPolicy`, ledger §P7.07). This is
  CITED, not implemented: the lifecycle is kernel-owned (another wave's
  crate) and this crate depends on no flexcore crate (§8.2 dependency rule,
  enforced by `tools/check_dependency_rule.py`).

DEFAULT decision (recorded): the empty ceiling is the posture of a deployment
that has composed no profile — every declared permission is
`PERMISSION_ABOVE_PROFILE` against it, so a profile-less host instantiates
capability-less components only, or nothing. The binary root
(`apps/plugin-host/src/main.rs`) composes exactly this empty ceiling: no
deployment profile exists in this wave (composition profiles are the P7.15
seam, owned by the kernel/lifecycle wave).

## 5. The drift-refusal rule (the alignment seam, host-side fail-closed)

The recorded seam (decision record §6 of `10_11_plugin_host_broker.md`):
kernel manifest `resources` spell `timeout-ms`; ledger §P7.18/§P7.19 prose
says `timeoutMs`; the kernel `ResourceBounds` carries NO fuel and NO
concurrency field. The kernel side was NOT touched (not this wave's
ownership). Host-side, the rule is: **ANY unalignable field is a refusal with
the named `PERMISSION_VOCABULARY_DRIFT` error — never a default
interpretation.** Concretely: the request's declared resource-field names
must all be in `RESOURCE_FIELD_VOCABULARY` (`max-memory-mb`, `timeout-ms`,
`max-output-mb`, `max-calls-per-minute`; values read 2026-11-13 from
`crates/flexcore-kernel/src/manifest.rs::parse_resources`); anything else —
`timeoutMs`, `fuel-ms`, `fuel`, `concurrency` — refuses the whole request.
The fuel budget therefore remains MANDATORY DEPLOYMENT CONFIGURATION (no
default), and concurrency stays the DEFAULT cap of 1 (record §3 of the P7.10
record). If the kernel vocabulary later changes, the mirror must be
deliberately updated; until then the gate refuses what it cannot align
(fail-closed in both directions).

## 6. The named-error tables

Enforcement (`enforcement.rs::EnforcementError::name`):
`PLUGIN_ID_INVALID`, `INSTANTIATION_REQUEST_INVALID`, `PERMISSION_UNKNOWN`,
`PERMISSION_NEVER_GRANTABLE`, `PERMISSION_DUPLICATE_DECLARATION`,
`PERMISSION_ABOVE_PROFILE`, `PERMISSION_VOCABULARY_DRIFT`, `PROFILE_INVALID`,
`LIMITS_INVALID`, plus the passthrough names (`BrokerRefusal` → the carried
`BrokerError::name`, `HostRefusal` → the carried `HostError::name`) and
`AUDIT_UNAVAILABLE`.

Enforcement audit kinds (`EnforcementAuditKind::name`):
`PLUGIN_INSTANTIATION_ALLOWED`, `PLUGIN_INSTANTIATION_REFUSED`,
`PLUGIN_CAPABILITY_SET_FROZEN`.

## 7. The negative suite: host-API-proven vs real-component (honest WRITTEN-UNRUN split)

All tests below are WRITTEN-UNRUN. The split is by what the fail-closed seams
CAN prove: with `NoBindings`, the attack surface is the HOST API, and the
tests prove the host refuses before any component code would run. Anything
requiring a live wasmtime instance is a HUMAN GATE.

| # | Attack | Proven at the host API (test, WRITTEN-UNRUN) | Needs a REAL wasmtime component (HUMAN GATE) |
|---|---|---|---|
| 1 | Filesystem read outside granted prefix (`..`, traversal, absolute, sibling prefix) | YES — `sandbox_p7_13.rs::a_path_traversal_request_is_refused_inside_a_granted_prefix`, `absolute_paths_and_sibling_prefixes_are_outside_the_grant` (`SCOPE_OUTSIDE_GRANT`) | Symlink-resolution semantics of a real WASI filesystem import — none exists here (zero `wasi:` imports); if a future world adds one, re-run with a real link inside the mount |
| 2 | Symlink-style escape | YES at the scope layer — `the_granted_prefix_is_a_virtual_mount_not_a_filesystem_escape` (a link-shaped name outside the prefix is refused; the prefix is a host-controlled virtual mount) | Real link resolution inside a real mount (none exists in this build) |
| 3 | Network sockets | YES — `network_sockets_are_unreachable_through_any_path` (`network.any` → `PERMISSION_NEVER_GRANTABLE` at grant AND check; socket-shaped ids → `PERMISSION_UNKNOWN`) | A real component attempting a socket import — no network import exists in any world; empty-linker behaviour over a real build |
| 4 | Environment variables | YES — `environment_variables_are_unreachable` (`PERMISSION_UNKNOWN`; no id exists) | A real component probing a WASI env import — no WASI support compiled in (DEFAULT, record §3 of the P7.10 record) |
| 5 | Wall clock | YES — `the_wall_clock_is_unreachable` (`PERMISSION_UNKNOWN`; the broker reads no clock — `now_unix_ms` parameter discipline, lexically pinned) | A real component probing a clock import — none exists |
| 6 | Subprocess spawn | YES — `subprocess_spawn_is_unreachable` (`PERMISSION_UNKNOWN`; device-protocol ids too) | A real component probing `wasi:cli/process` — no WASI support compiled in |
| 7 | Memory limit exceeded | TYPE-LEVEL — `every_exhaustion_error_carries_the_teardown_contract` (named error + teardown contract text); the declared page limit is re-validated and applied at the gated store construction (`bindings.rs` §feature-gated) | REAL RUN: a component allocating past its page limit is torn down, host survives, teardown audited (`40_toolchain_pinning.md` §5 commands) |
| 8 | Fuel exhausted mid-call | TYPE-LEVEL + DOOR — same contract test; an unmetered instance is not constructible (fuel mandatory at the door, `fuel_exhaustion_is_designed_away_at_the_door_not_configured_away`) | REAL RUN: a runaway computation hits `FUEL_EXHAUSTED` mid-call |
| 9 | Epoch deadline exceeded | TYPE-LEVEL — `the_epoch_deadline_is_derived_from_the_declared_timeout_only` (ticks = ceil(timeout/10 ms), one source of truth) | REAL RUN: a looping guest yields at the next epoch boundary and is torn down |
| 10 | Output limit exceeded | TYPE-LEVEL — contract test; `OUTPUT_LIMIT_EXCEEDED` carries "call refused; instance torn down" | REAL RUN: a multi-GB-returning component is cut off |
| 11 | Call rate exceeded | TYPE-LEVEL — contract test; declared rate governs (`output_and_call_rate_defaults_are_caps_not_permissions`) | REAL RUN: an abusive caller is refused and torn down |
| 12 | Ungranted / revoked / expired capability use | YES — `calling_an_ungranted_capability_is_refused_and_audited`, `a_revoked_grant_cannot_be_reused`, `an_expired_grant_is_refused` | (none — fully host-API) |
| 13 | Grant forgery | YES, STRUCTURAL + BEHAVIOURAL — `grant_forgery_is_structurally_refused_and_behaviourally_futile` (grant type broker-constructed; captured grant inert after revocation; captured object does not seed a fresh broker) | (none — fully host-API) |
| 14 | Dispatch bypass (ADR-010) | YES — §8's tests (refused at gate AND broker; exhaustive id sweep) | (none — fully host-API; the plugin is UNBUILDABLE) |
| 15 | Resource-bound lie | YES — `the_host_enforces_the_declared_bounds_not_the_components_belief` (declared bounds mirrored + re-validated; no widening API) | REAL RUN: a component "behaving" larger than its declared memory/timeout is cut off by the store-level limiter/fuel/epoch machinery |
| 16 | Wasmtime-internal sandbox escape (CVE surface) | NOT PROVABLE HERE | HUMAN GATE (§9): the R7-C2 CVE floor, advisory re-check, and a real hostile-component run |

Human-gate run list (post-first-build, owner: P7.13's verifier): build with
`cargo test -p flexcore-plugin-host`; then, once the `generated-bindings`
feature and a real signed component exist (P7.24's example owner), run the
hostile components of ledger §P7.13 step 1 (files, sockets, env, unbounded
allocation, infinite loop, multi-GB output, abusive call rate) against the
real host and record: denied / terminated by limit / host unaffected /
teardown audited — per `40_toolchain_pinning.md` §5's prepared commands.

## 8. The ADR-010 unbuildable proof (dispatch bypass)

ADR-010 / scope-freeze P-4: "the SDK must make a dispatch-bypassing plugin
UNBUILDABLE, not merely discouraged". The proof is the composition of four
structural fences, each tested:

1. The permission vocabulary contains no authorization id —
   `dispatch.authorize`/`dispatch.issue` are in the never-granted set
   (`broker.rs::NEVER_GRANTED_PERMISSIONS`), refused by the kernel manifest
   gate first, then RE-REFUSED at the enforcement door (evaluate step 4) and
   at the broker's grant AND check paths (tests:
   `a_dispatch_bypassing_manifest_is_refused_at_the_door_and_at_the_broker`).
2. A "validation verdict" authority does not exist to request — any crafted
   id outside the closed vocabulary is `PERMISSION_UNKNOWN`
   (`a_validation_verdict_authority_does_not_exist_to_request`).
3. The one dispatch-adjacent id (`dispatch.validate.request`) is fenced to
   exactly the `protected-validator` target at grant time, and the forward
   returns `()` — no verdict type exists in any signature a plugin can reach
   (`the_validate_request_capability_can_only_target_the_protected_validator`,
   `the_validation_forward_returns_no_verdict_type_level`).
4. The exhaustive sweep: every grantable id yields proposals or request-
   forwards only; no verdict can contain a capability outside the allowed
   set, and none can be `dispatch.authorize`/`dispatch.issue`
   (`no_permission_id_conveys_dispatch_authority_unbuildable_not_discouraged`).

A dispatch-bypassing plugin therefore has no buildable permission
configuration in this host — it is refused at the door before any component
code runs, and no alternative path exists.

## 9. Wasmtime version and the R7-C2 human-gate item (P7.13 steps 3–4)

The pinned version is `wasmtime = "=50.0.1"` (`apps/plugin-host/Cargo.toml`
D4 note), exact per `40_toolchain_pinning.md` §2, carrying the three October
2026 fixes: CVE-2026-115889 (GHSA-897f-g5gm-fmxm), CVE-2026-115893
(GHSA-mghf-wjfp-p5xm), CVE-2026-115384. **HUMAN GATE (recorded, not
performed here):** re-check the three GHSA advisory pages at implementation
time, record the advisory IDs and the check date, and confirm the pinned
version carries all three fixes (RESEARCH.md §7 task 1; the 03 audit table
flags wasmtime RE-VERIFY). The sandboxing caveat (ledger §P7.13 step 3, §5.4
[R4]) stands: wasmtime provides sandboxing mechanisms, but sandboxing must be
combined with host capability policy (this wave), resource limits (P7.10),
signatures (P7.14), and isolation (§8.3 separate process). WASM-level escape
surface testing beyond the host API is a first-build verification item, not
coverage this loop claims.

## 10. File inventory (ownership) — including two flagged necessary edits

NEW (this wave):
`apps/plugin-host/src/enforcement.rs`,
`apps/plugin-host/tests/enforcement_p7_12.rs`,
`apps/plugin-host/tests/sandbox_p7_13.rs`, this record.

EDITED (additive/visibility-only, as planned):
`apps/plugin-host/src/lib.rs` (one module line + one module-table row);
`apps/plugin-host/src/host.rs` (`bind_component` `pub` → `pub(crate)`,
VISIBILITY-ONLY — body untouched; the demotion IS the no-raw-path rule);
`apps/plugin-host/src/broker.rs` (`CapabilityGrant` fields `pub` →
`pub(crate)` + additive getter methods — VISIBILITY-ONLY, no logic change;
required by the P7.13(c) forgery refusal "the grant type's constructor is
private to the broker").
`ledger/phase-07-plugin-sdk/ledger_phase_7.md` (ticks for P7.12/P7.13 ONLY).

EDITED out of necessity (flagged honestly; required for the crate to compile
after the mandated no-raw-path change — call-site migration only, no logic
rewritten):
`apps/plugin-host/src/main.rs` — the binary now composes the enforcement
gate (empty DEFAULT ceiling) and instantiates through `enforcement::instantiate`;
three new required identity variables (`FLEXCORE_PLUGIN_HOST_PLUGIN_ID`,
`FLEXCORE_PLUGIN_HOST_MANIFEST_ID`, `FLEXCORE_PLUGIN_HOST_MANIFEST_VERSION`)
and one optional permissions variable (`FLEXCORE_PLUGIN_HOST_PERMISSIONS`);
`apps/plugin-host/tests/host_p7_10.rs` — the bindings-seam test reaches the
seam through the verdict path instead of the demoted helper;
`apps/plugin-host/tests/broker_p7_11.rs` — five grant-field accesses moved
to the new getters, PLUS a pre-existing WRITTEN-UNRUN defect fixed: two
tests moved the audit sink into the broker (`Box::new(sink)`) and read it
afterwards — a use-after-move (E0382 at the first human build). The minimal
fix is a `SharedSink` wrapper (`Arc<InMemoryBrokerAuditSink>`, clone shares
the same log) added in the TEST file only; no broker logic changed.

NOT TOUCHED: `crates/flexcore-kernel/**` (read-only this wave), the ledger
P7.14/P7.15/P7.16 tick lines, `docs/foundations/*`, `docs/plan/*`, `.agent/*`,
`wit/*`, root `Cargo.toml`, `tools/check_dependency_rule.py`.

## 11. Acceptance check (ledger P7.12 / P7.13 — honest)

P7.12:
- "Every §5.7 row is enforced for every plugin class (matrix fully filled
  with code locations)": **CLASS B FILLED; CLASS A/C RECORDED AS SEAMS** —
  the Class A cells are the kernel registry's (P7.08, another wave) and the
  Class C cells are the worker-template seams (P7.17/P7.20, not built in
  this loop). Recorded, not faked (the ledger's own risk names this exact
  half-enforcement gap).
- "All escalation attempts fail and are audited": **DESIGNED, WRITTEN-UNRUN**
  — the escalation tests exist (`enforcement_p7_12.rs`, `sandbox_p7_13.rs`);
  the audit-or-refuse chain covers both directions including grant rollback.
- "A proposed schedule never becomes an issued command without the protected
  path": **STRUCTURALLY ENFORCED** (no authorization id exists; the forward
  returns `()`), tested WRITTEN-UNRUN. Tracing one proposed schedule through
  validation/feasibility/authorization (ledger verification method) needs the
  real control-plane composition — recorded as a human-gate observation.

P7.13:
- "All hostile components fail safely; the host process survives every case":
  **NOT RUN — no hostile component exists and no build was executed.** The
  type-level teardown contracts and the host-API refusals are WRITTEN-UNRUN;
  the behavioural record is the human-gate list (§7).
- "No ambient resource is reachable": **STRUCTURALLY CLOSED TODAY** (zero-
  `wasi:` worlds, empty linker, closed vocabulary — all pinned by tests) and
  behaviourally proven only at the human gate.
- "The Wasmtime version is recorded and is ≥ the CVE-fixed release":
  RECORDED (=50.0.1 with the three advisory IDs); the advisory re-check is
  the §9 human gate.

Test status: `enforcement_p7_12.rs` (17 tests) and `sandbox_p7_13.rs` (21
tests) are WRITTEN-UNRUN. No "tested"/"verified"/"passed" claim is made
anywhere; the first human `cargo test -p flexcore-plugin-host` run records
the outcomes.

## 12. Related records

- `10_11_plugin_host_broker.md` — the host, the broker, the alignment seam
  (§6) whose drift behaviour this wave turns into a named refusal.
- `05_06_manifest_resolution.md` — the manifest schema and D-P7.05-6 (scope
  at grant time).
- `07_08_lifecycle_registry.md` — drain-before-replace (the cited
  re-instantiation path).
- `02_scope_freeze.md` / `04_seam_inventory.md` — the protected non-seams
  (CommandAuthorizer, DispatchGateway, final dispatch validation) that the
  ADR-010 proof leans on.
- `40_toolchain_pinning.md` — the wasmtime pin, the CVE floor, the prepared
  human-build commands.
- `ledger_phase_7.md` §P7.29, §P7.34 — the consumers of these negatives
  (security-test report; the host is an untrusted process from day one).
