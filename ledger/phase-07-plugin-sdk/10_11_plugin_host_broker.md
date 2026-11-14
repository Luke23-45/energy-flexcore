# P7.10/P7.11 — Wasmtime component host prototype and the host capability broker

Date: 2026-11-13. Subtasks: P7.10, P7.11
(`ledger_phase_7.md` §P7.10, §P7.11). Owner roles: Rust engineer (SDK) /
Security + Rust engineer. Report trace: §5.4 Class B, §5.7, §8.1, §8.3,
§17.3, §17.4, §21.4. Inputs: P7.09 (`09_wit_worlds.md`, the `wit/` tree);
P7.05/P7.06 (`05_06_manifest_resolution.md`, the manifest types and the
kernel permission model); P7.03 (`03_sdk_license_audit.md`); P7.40
(`40_toolchain_pinning.md`, the wasmtime 50.0.1 pin); RESEARCH.md §1/§6–§7
(R7-C2 CVE floor; the open verification tasks).

## 0. Honest status — READ FIRST

**Everything in this wave is WRITTEN-UNRUN.** No cargo command was executed
anywhere in this loop (no install, no build, no test): the wasmtime 50.0.1
pin has never been compiled, the bindings have never been generated, no
component artifact exists, and every test in
`apps/plugin-host/tests/` is WRITTEN-UNRUN. The first human build
(`cargo test -p flexcore-plugin-host`) is the verification gate for
everything below; until it is recorded, every claim about the code is a
design statement, not a tool-verified fact. Where the wasmtime API is
involved, every call site carries the marker
`// UNVERIFIED — wasmtime 50 API, first human build verifies` and lives
inside a compile-gated seam (§4) — the default build contains ZERO wasmtime
call sites.

## 1. What was built (per-subtask coverage)

| Subtask | Delivered | Where |
|---|---|---|
| P7.10 step 1 (host as a separate process) | The new app crate `apps/plugin-host` (package `flexcore-plugin-host`), a separate-process COMPOSITION crate; the host↔control-plane gRPC transport is recorded as the P7.20 seam and mTLS identity as the P7.34 seam — NO in-process linking exists or will be composed here (report §8.3) | `apps/plugin-host/` (member entry added to the root `Cargo.toml`, registered in `tools/check_dependency_rule.py`) |
| P7.10 step 2 (instantiate against the worlds, bind only declared imports) | The bindings seam ([`bindings`]) against the three recorded P7.09 worlds by their WIT paths; the linker starts empty and stays empty — the P7.09 worlds declare ZERO `wasi:` imports, and there is no ambient import of any kind | `apps/plugin-host/src/bindings.rs`, `apps/plugin-host/src/host.rs` (`WorldId`) |
| P7.10 step 3 (resource limits) | [`limits::ResourceLimits`] derived from the manifest's declared bounds: memory pages, mandatory fuel budget, wall-clock epoch deadline (wasmtime epoch pattern, UNVERIFIED), output size, call rate, concurrency (DEFAULT 1) — every limit exceeded is a named error with the instance-torn-down enforcement contract | `apps/plugin-host/src/limits.rs` |
| P7.10 step 4 (example component end to end) | NOT DONE — recorded honestly: no example component exists (the `plugins/wasm-examples/` materialisation is the P7.24 owner per `40_toolchain_pinning.md` §2), no artifact was loaded, no limit behaviour was observed. The limit-BEHAVIOUR record the ledger asks for requires a running host — P7.13's owner over a real build | — |
| P7.10 binary root | Thin fail-fast binary: unconfigured component/world/symbol → named error, exit 2; missing artifact or no bindings → named error, exit 1; tracing wired (env-filtered); the exit-0 path is unreachable in this loop by design | `apps/plugin-host/src/main.rs` |
| P7.11 steps 1–2 (vocabulary + broker) | [`broker::CapabilityBroker`]: closed permission vocabulary mirroring the kernel manifest gate (§6 below), explicit scoped grants, per-call re-checks, no cached ambient authority, defaults deny | `apps/plugin-host/src/broker.rs` |
| P7.11 step 3 (never-granted set) | `dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any` refused at grant time AND at check time, regardless of any manifest/profile; the validate-request capability is grantable ONLY with the protected-validator target | `broker.rs` (`NEVER_GRANTED_PERMISSIONS`, `validate_request_target`) |
| P7.11 step 4 (`dispatch.validate.request` boundary) | [`broker::CapabilityBroker::request_validation_forward`]: forwards the plugin's REQUEST to the protected validator and returns `()` — no verdict type exists in the signature, so no verdict can flow back through the broker; the protected validation gate is never substituted | `broker.rs` |
| P7.11 step 5 (denial logging) | Every grant, use, denial, revocation, and forward is audited through the [`broker::BrokerAuditSink`] port (style of the edge-agent audit events); audit-or-refuse — a broken sink stops the broker in both directions | `broker.rs` |

## 2. The file inventory (ownership)

NEW (this wave): `apps/plugin-host/Cargo.toml`, `apps/plugin-host/src/{lib,host,bindings,limits,broker,main}.rs`,
`apps/plugin-host/tests/{host_p7_10,broker_p7_11}.rs`. EDITED (additive
only): the root `Cargo.toml` (the member entry) and
`tools/check_dependency_rule.py` (the `flexcore-plugin-host` composition-crate
registration). NOT TOUCHED: `crates/flexcore-kernel/**` (another agent's
ownership; the alignment seam is §6), `docs/foundations/*`, `docs/plan/*`,
`.agent/*`, `wit/*`, `rust-toolchain.toml`.

## 3. The wasmtime pin and feature choices (each UNVERIFIED)

`apps/plugin-host/Cargo.toml` carries the D4 note block verbatim (license
status, the three CVE rows, the never-built-here status). The choices:

| Choice | Value | Status |
|---|---|---|
| Version pin | `wasmtime = "=50.0.1"` — EXACT. The CVE floor (≥ 50.0.0) is a minimum; the pin is exact per the recorded policy (`40_toolchain_pinning.md` §2). 50.0.1 carries all three October 2026 fixes (CVE-2026-115889, CVE-2026-115893, CVE-2026-115384) | UNVERIFIED until the human cargo-deny run (D4 open; the 03 audit table flags wasmtime RE-VERIFY) |
| Where the pin lands | In `apps/plugin-host/Cargo.toml` directly — this wave's root-Cargo.toml edit is the additive member entry ONLY (shared-file rule). Promoting the pin to the root `[workspace.dependencies]` is the first human build step per `40_toolchain_pinning.md` §2 ("Where it lands") | DEFAULT (recorded here, not silently absorbed) |
| `default-features = false` | Fail-closed minimisation: nothing is enabled that was not chosen | DEFAULT |
| `cranelift` | The JIT backend required to instantiate a component; Winch/interpreter are not recorded pins and are NOT enabled | UNVERIFIED — wasmtime 50 feature name, first human build verifies |
| `runtime` | The runtime/linker machinery under the Component Model embedding API | UNVERIFIED — wasmtime 50 feature name, first human build verifies |
| `component-model` | The WIT/Component Model embedding API the host uses | UNVERIFIED — wasmtime 50 feature name, first human build verifies |
| WASI host support | **DELIBERATELY NOT ENABLED** (DEFAULT): the P7.09 worlds carry zero `wasi:` imports (`wit/README.md` §5 item 8), and a WASI host implementation would create exactly the ambient capability surface P7.11 forbids. The capability broker, not WASI, is the grant mechanism for any future host-provided interface (`wit/README.md` §2 design-for-host note). The `wasmtime-wasi` row in `03_sdk_license_audit.md` therefore stays UNPINNED-REVERIFY and unconsumed | DEFAULT (fail-closed direction) |
| Other dependencies | None: the non-wasmtime code of the crate is std + tracing (+ tracing-subscriber in the binary root) only | recorded |

## 4. The feature-gated bindings design (the honest compile story)

The problem: the wasmtime 50 Component Model embedding API has never been
compiled in this loop, and no `.wasm` artifact exists. Writing direct
wasmtime call sites into always-compiled code would make the whole crate's
compilability (and honesty) depend on API shapes nobody has ever built.

The design (`apps/plugin-host/src/bindings.rs`):

- The host logic (`host.rs`, `limits.rs`, `broker.rs`) is std + tracing only
  and compiles WITHOUT any wasmtime call site.
- Every point where wasmtime is invoked lives in
  `#[cfg(feature = "generated-bindings")] mod wasmtime_bindings`, compiled
  ONLY when the human's `cargo component` / `wit-bindgen` run has
  materialised the generated modules over the `wit/` tree (a BUILD-TIME
  step — the host references the recorded worlds by path and never
  re-declares the WIT contract in code).
- The DEFAULT build compiles [`bindings::NoBindings`], a fail-closed
  implementation of the [`bindings::PluginBinding`] trait whose every use
  returns the named `BINDINGS_NOT_GENERATED` error. The crate therefore
  compiles without wasmtime artifacts while remaining honest: it can load
  an artifact path, refuse it if absent, and refuse to instantiate until
  the real build exists.
- Enabling `generated-bindings` WITHOUT materialising the generated module
  is a loud compile error — the gate is not a magic switch; the feature is
  not in the default feature set.
- The gated module documents the expected per-world structure, the
  linker-stays-empty rule (no ambient import, ever), and the three
  per-store limit application points (resource limiter → memory pages;
  `set_fuel` → the mandatory budget; `set_epoch_deadline` → the wall-clock
  deadline). Its `verify()` also reports `BINDINGS_NOT_GENERATED` until the
  first human build replaces the body with the real typed-interface check —
  fail-closed, never a guessed `Ok`.

The epoch-based wall-clock deadline pattern (documented from the wasmtime
docs; UNVERIFIED — wasmtime 50 API, first human build verifies): a host
ticker calls `Engine::increment_epoch()` every `EPOCH_TICK_MILLIS` = 10 ms
(DEFAULT decision); the store sets
`set_epoch_deadline(ResourceLimits::epoch_ticks())` (ticks = ceiling of
`timeout_ms / EPOCH_TICK_MILLIS`, sub-tick remainders round UP — a guest
never gets a free extra tick). A guest that runs past its declared deadline
yields at the next epoch boundary and is torn down; the host process is
never blocked by a looping guest (the §P7.13 "loops forever" hostile
component fails safely against this).

## 5. The named-error tables

Host (`apps/plugin-host/src/host.rs::HostError::name`):
`COMPONENT_UNCONFIGURED`, `COMPONENT_NOT_FOUND`, `COMPONENT_UNREADABLE`,
`COMPONENT_OVERSIZED`, `BINDINGS_NOT_GENERATED`, `WORLD_UNKNOWN`,
`LIMITS_INVALID`, `MEMORY_LIMIT_EXCEEDED`, `DEADLINE_EXCEEDED`,
`FUEL_EXHAUSTED`, `OUTPUT_LIMIT_EXCEEDED`, `CALL_RATE_EXCEEDED`.

Broker (`apps/plugin-host/src/broker.rs::BrokerError::name`):
`PERMISSION_UNKNOWN`, `PERMISSION_NEVER_GRANTABLE`, `CAPABILITY_NOT_GRANTED`,
`GRANT_EXPIRED`, `SCOPE_OUTSIDE_GRANT`, `PERMISSION_NOT_DECLARED`,
`DUPLICATE_GRANT_REQUEST`, `GRANT_EXPIRY_INVALID`, `SCOPE_INVALID`,
`PLUGIN_ID_INVALID`, `GRANT_UNKNOWN`, `AUDIT_UNAVAILABLE`.

Audit kinds (`broker.rs::BrokerAuditKind::name`): `PLUGIN_CAPABILITY_GRANTED`,
`PLUGIN_CAPABILITY_REVOKED`, `PLUGIN_CAPABILITY_USED`,
`PLUGIN_CAPABILITY_DENIED`, `PLUGIN_VALIDATION_FORWARD_REQUESTED`.

## 6. The manifest-alignment seam (kernel side untouched)

The broker's permission vocabulary is a documented MIRROR of
`crates/flexcore-kernel/src/manifest.rs` (`ALLOWED_PERMISSIONS`,
`NEVER_GRANTED_PERMISSIONS` — 6 allowed ids, 4 never-granted ids, values
read 2026-11-13 from that file; the kernel crate was NOT touched, it is
owned by the kernel/lifecycle wave). The mirror exists because this crate's
non-wasmtime code is deliberately std + tracing only, so the kernel crate
(and its serde_json dependency) is not imported.

The same applies to the resource bounds: [`limits::DeclaredBounds`] mirrors
the kernel's `ResourceBounds` struct (field names and closed ranges:
`max-memory-mb` 1..=65_536, `timeout-ms` 1..=3_600_000, `max-output-mb`
1..=4_096, `max-calls-per-minute` 1..=1_000_000) and the host re-validates
against the SAME ranges.

Drift behaviour (recorded, fail-closed in both directions): if the kernel
vocabulary ever changes without this mirror following, the broker refuses
anything not in ITS closed vocabulary — a kernel-side addition would be
denied here until the mirror is deliberately updated. An enforcement gap is
not reachable through drift; a functional stall is, and it fails loudly.

Open alignment item for the kernel wave (NOT acted on here):
`ledger_phase_7.md` §P7.18 and §P7.19 reference manifest
`resources.timeoutMs`; the kernel schema spells it `timeout-ms` and its
`ResourceBounds` carries NO fuel/concurrency field. The fuel budget is
therefore DEPLOYMENT CONFIGURATION in this host (mandatory, no default —
an unmetered instance is refused at limits construction), and concurrency
defaults to 1 (DEFAULT). If the kernel later declares fuel/concurrency in
the manifest, this mirror and the limits construction follow (a kernel-side
change is out of this wave's ownership).

## 7. The no-ambient-access capability table

Defaults deny. Every grant is explicit, scoped, expiry-bounded, and bound
to one plugin and one manifest id+version; revocation is dropping the grant
and takes effect at the next call (per-call re-check, no cached authority).
The scope vocabulary: `ReadOnlyPathPrefix` (segment-boundary containment,
`..` refused both at grant time and at check time), `SendOnlyTarget`
(exact target match), `DataScope` (tenant → optional site → optional asset,
no widening).

| Ambient resource | Exposed to a component? | Mechanism that keeps it closed |
|---|---|---|
| Filesystem | NO, unless a scoped `ReadOnlyPathPrefix` grant exists AND the manifest declares `telemetry.read`/`asset.read` AND the grant is recorded | The P7.09 worlds declare no `wasi:filesystem` import; the gated linker starts empty and stays empty; broker scope containment (segment boundary + `..` refusal) |
| Network | NEVER (`network.any` is in the never-granted set) | No network import in any world; no broker path can grant it; the P7.09 world inventory carries zero network surfaces |
| Clock | NO — the broker itself reads no clock (time is a caller-supplied parameter) and no clock import exists in any world | World inventory + broker parameter discipline |
| Environment variables | NO — no capability id exists for them; a request is `PERMISSION_UNKNOWN` | Closed vocabulary |
| Secrets | NEVER as values (`secret.read` never granted) — references resolve host/adapter-side (P7.19); the broker never sees a value and its audit details are identifiers/prefixes only | Never-granted set + P7.19 seam |
| Device protocols / direct device access | NEVER — not a grantable capability; §5.4's Class B boundary | Closed vocabulary + the protected-kernel contract guard (kernel resolver) |
| `dispatch.authorize` / `dispatch.issue` | NEVER — never-granted; a crafted manifest declaring them is refused at derivation (`PERMISSION_NEVER_GRANTABLE`) | Kernel manifest gate + broker re-refusal |
| `dispatch.validate.request` | Send-only to exactly `protected-validator`; the forward returns no verdict (type-level: `()`); the protected gate is never substituted | `validate_request_target` + the forward signature |

The P7.12 enforcement matrix consumes this broker (its step 1 names code
locations per §5.7 row × plugin class); the P7.13 hostile suite targets
this host (its step 1 components exercise the table above over a REAL
build); P7.18 worker supervision and P7.34 trust boundary build on the
separate-process posture (the host is an UNTRUSTED-HOST design from day
one — no result it produces is trusted input to the control plane).

## 8. Acceptance check (ledger P7.10 / P7.11 — honest)

P7.10:
- "Example component runs and returns a §9.2-conformant result": **NOT MET**
  — no example component exists and no build was run (P7.24 owns the
  example; the first human build owns the run). Recorded, not faked.
- "A component that exceeds a limit is terminated cleanly with an audit
  event (not a host crash)": **DESIGNED, UNPROVEN** — the named-error +
  torn-down contracts are in code and tested at the type level
  (WRITTEN-UNRUN); the behavioural proof is the P7.13 owner's over a real
  build.
- "No ambient access is available to the component (tested in P7.13)":
  **STRUCTURALLY CLOSED TODAY** (zero-`wasi:` worlds, empty linker, closed
  vocabulary) and behaviourally proven in P7.13 over a real build.

P7.11:
- "A component requesting an ungranted capability is denied with an audit
  event": implemented (`CAPABILITY_NOT_GRANTED` + `PLUGIN_CAPABILITY_DENIED`);
  tested WRITTEN-UNRUN.
- "A component with only `telemetry.read` cannot call `schedule.propose`":
  implemented (per-capability grants); tested WRITTEN-UNRUN.
- "No capability in the never-granted set is grantable through any manifest
  or profile": implemented (grant-time + check-time re-refusal, plus the
  kernel manifest gate ahead of both); tested WRITTEN-UNRUN including the
  crafted-manifest case.

Test status: `apps/plugin-host/tests/host_p7_10.rs` (14 tests),
`apps/plugin-host/tests/broker_p7_11.rs` (22 tests), and the two in-module
limits tests are WRITTEN-UNRUN. No "tested"/"verified" claim is made
anywhere; the first human `cargo test -p flexcore-plugin-host` run records
the outcomes.

## 9. Related records

- `40_toolchain_pinning.md` — the wasmtime 50.0.1 pin, the CVE floor, and
  the PREPARED-UNRUN human validation commands.
- `03_sdk_license_audit.md` — the wasmtime license/CVE rows this crate's
  Cargo.toml D4 note carries.
- `05_06_manifest_resolution.md` — the manifest schema, the class-B entry
  point (`symbol`), the permission model, and D-P7.05-6 (scope at grant
  time).
- `09_wit_worlds.md` + `wit/README.md` — the worlds the host instantiates
  against (zero `wasi:` imports).
- `ledger_phase_7.md` §P7.12, §P7.13, §P7.18, §P7.20, §P7.34 — the
  consumers of this host and broker (permission enforcement, sandbox
  negatives, worker supervision, gRPC template, trust boundary).
