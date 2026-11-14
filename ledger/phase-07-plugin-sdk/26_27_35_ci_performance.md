# P7.26 + P7.27 + P7.35 — Perf harness, CI gates to the SDK, no-dylib gate: IMPLEMENTED (awaiting human build/test/execution)

Date: 2026-11-13. Subtasks: P7.26, P7.27, P7.35
(`ledger_phase_7.md` §P7.26, §P7.27, §P7.35). Owner role: DevOps.

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-07-plugin-sdk/ENTRY_NOTE.md`):** every deliverable in this
wave is WRITTEN, NOT BUILT, NOT RUN. No cargo, buf, wasm-tools, or CI
command of any kind was executed. Every check, gate, measurement, and green
run below is **UNRUN** until the human executes it; the measured and
verdict cells are deliberately EMPTY. The ONLY executions in this loop were
the three permitted repo tools (output in §8) and the lexical baseline
greps (their actual output is quoted in §3 — reading source files, no
build, no test).

## 0. Files (complete list; nothing outside it was touched)

| File | Status |
|---|---|
| `apps/plugin-host/tests/perf_harness_p7_26.rs` | NEW (P7.26) |
| `apps/plugin-host/tests/no_dylib_p7_35.rs` | NEW (P7.35) |
| `python/workers/contracts/buf.yaml` | NEW (P7.27; did not exist — checked before writing) |
| `python/workers/contracts/buf.gen.yaml` | NEW (P7.27; did not exist — checked before writing) |
| `.github/workflows/ci.yml` | EDITED (additive: one header comment block, five new jobs `plugin-sdk-conformance`, `plugin-manifest-validation`, `wit-validate`, `worker-contract-buf`, `no-dylib-gate`; NO existing job or step rewritten — `git diff` shows 449 insertions, 0 deletions) |
| `deny.toml` | **NOT TOUCHED** — no entry was structurally required (§5) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.26/P7.27/P7.35 ticks ONLY — P7.24/P7.25 belong to the concurrent wave; their lines were not touched) |
| this file | NEW (decision record) |

No `crates/**` or `apps/plugin-host/src/**` file was touched (the crate
sources are CLOSED to this wave; the concurrency constraint holds — the
P7.24/P7.25 wave was in flight in parallel and is not owned by this wave).

---

## 1. P7.26 — performance and resource validation (TEMPLATES, no invented numbers)

**Status: HARNESS + EMPTY REPORT TEMPLATES ONLY. Nothing was measured.**
No number exists anywhere in this record; every measured cell below is
EMPTY and each names the EXACT command/test that fills it post-first-build.

### 1.1 What the harness measures today (fail-closed seams, no component needed)

`apps/plugin-host/tests/perf_harness_p7_26.rs` is a std-only harness
(`std::time::Instant`; NO criterion, NO new dependencies — the P6.22
`perf_harness_p6_22.rs` pattern) over the four host-side overheads that
exist WITHOUT a WASM component:

| Seam | Measured call | What the timing includes |
|---|---|---|
| Enforcement-gate evaluation cost | `enforcement::EnforcementGate::evaluate` | the P7.12 door: manifest ∩ profile-ceiling intersection + broker grant derivation + audit emission (fresh gate/sinks per iteration; `evaluate` timed) |
| Broker grant derivation | `broker::CapabilityBroker::derive_grants` | the P7.11 derivation: vocabulary + declared-permission checks, scope validation, per-grant audit (fresh broker per iteration) |
| Limits construction | `limits::ResourceLimits::from_declared_bounds` | the closed-range re-validation + exact page/epoch derivations |
| Health snapshot serialization | `health::PluginHealthMonitor::snapshot` + `HealthSnapshot::json` | the P7.25 console payload over 8 registered/observed plugins (setup untimed) |

Every assertion in the harness is STRUCTURAL (the seam answers; the page
conversion is exactly 16 pages/MiB; `epoch_ticks == epoch_ticks_for(deadline_ms)`;
the fail-closed refusal is the named one) — NO timing threshold, §19.4.

### 1.2 The real measurements — recorded, SKIPPED-WHEN-NO-BINDINGS

The harness prints a `flexcore-pluginhost-perf-record/v1` block. Under the
DEFAULT build the bindings seam is `NoBindings` (fail-closed), so the
component-side rows print `SKIPPED` with their filling command; the harness
deliberately does NOT write the `generated-bindings` code arm (it belongs
to the owner of the human's wit-bindgen build — enabling the feature
without that arm materialised is the crate's recorded loud compile error).
The one thing the default build CAN prove structurally is recorded as a
row: `bindings_seam_state = fail-closed (BINDINGS_NOT_GENERATED on every
use)` — driven through the real `enforcement::instantiate` →
`verify()` seam with a throwaway artifact.

### 1.3 The MEASURED report — the deliverable — ALL CELLS EMPTY (UNRUN)

Fill from the printed record blocks, per profile, with the hardware/OS/
versions/conditions of each run. **x86-64 AND ARM64 are two separate
measured runs.** Lab-vs-target honesty: the harness host is NOT the target
machine — test-host numbers are context, never target figures.

| Measurement | x86-64 (measured) | ARM64 (measured) | Filled by (EXACT command/test) |
|---|---|---|---|
| Component instantiation time | (EMPTY — not measured) | (EMPTY — not measured) | `cargo test -p flexcore-plugin-host --test perf_harness_p7_26 --features generated-bindings -- --nocapture` (after the human's wit-bindgen run materialises the bindings modules per `wit/TOOLCHAIN.md`, against a signed component artifact) |
| Per-call overhead (WASM call latency p95/p99) | (EMPTY — not measured) | (EMPTY — not measured) | same harness, same preconditions |
| Memory per component | (EMPTY — not measured) | (EMPTY — not measured) | same harness + EXTERNAL memory observation (not measurable from inside a Rust test process — human gate input) |
| Fuel consumption per contract call | (EMPTY — not measured) | (EMPTY — not measured) | same harness, same preconditions (fuel accounting is the gated implementation's) |
| Epoch-deadline behaviour under load | (EMPTY — not measured) | (EMPTY — not measured) | same harness + the limit-hit load drill (P7.13 §7 / P7.30) |
| Enforcement-gate evaluation cost (p50/p95/p99) | (EMPTY — not measured) | (EMPTY — not measured) | `cargo test -p flexcore-plugin-host --test perf_harness_p7_26 -- --nocapture` (prints `enforcement_gate_evaluation_ns` percentiles) |
| Broker grant derivation cost (p50/p95/p99) | (EMPTY — not measured) | (EMPTY — not measured) | same run (prints `broker_grant_derivation_ns`) |
| Limits construction cost (p50/p95/p99) | (EMPTY — not measured) | (EMPTY — not measured) | same run (prints `limits_construction_ns`) |
| Health snapshot serialization cost (p50/p95/p99) | (EMPTY — not measured) | (EMPTY — not measured) | same run (prints `health_snapshot_serialization_ns`) |
| Host process memory/CPU under load | (EMPTY — not measured) | (EMPTY — not measured) | external observation on the target during the load drill (human gate input) |
| gRPC worker call latency p95/p99 | (EMPTY — not measured) | (EMPTY — not measured) | the P7.28/P7.30 lab run against the example worker (`python/workers/`); no gRPC harness exists in this wave — recorded honestly, not invented |
| Supervisor timeout behaviour | (EMPTY — measured cells) | (EMPTY — measured cells) | behaviour determinism: `cargo test -p flexcore-worker --test supervision_p7_18 -- --nocapture` (the P7.18 injected-clock suite — REFERENCED, not duplicated; wall-clock measured cells need the P7.30 lab run) |
| Backpressure engagement point | (EMPTY — not measured) | (EMPTY — not measured) | deterministic engagement: `supervision_p7_18`; queue-depth metrics under real saturation: the P7.18 fault-injection verification / P7.30 lab run |
| Recovery drain time | (EMPTY — not measured) | (EMPTY — not measured) | the P7.30 failure-mode drill (lab; recovery time from the logs) |

### 1.4 Load profile — from pilot/research deployment — PLACEHOLDER

| Profile parameter | Value |
|---|---|
| Invocations per schedule run | PLACEHOLDER pending pilot/research input |
| Component and worker counts | PLACEHOLDER pending pilot/research input |
| Payload sizes | PLACEHOLDER pending pilot/research input |
| Failure-case mix (fuel/time limit hits, timeouts) | PLACEHOLDER pending pilot/research input |

The harness's iteration counts (1_000/1_000/10_000/2_000) are HARNESS
PARAMETERS chosen to exercise the paths in seconds — they are NOT the load
profile. **No fleet extrapolation (§19.3):** nothing in this record
projects any measured value beyond the tested profile, and none exists yet.

---

## 2. P7.27 — CI gates extended to the SDK

### 2.1 The §22.2 gate matrix (gate → job → status)

| §22.2 gate | CI wiring | Covers the Phase 7 crates? | Status |
|---|---|---|---|
| `cargo fmt --check` | existing `static-checks` job (UNCHANGED) | Yes — workspace-wide; `flexcore-kernel` and `flexcore-plugin-host` are members | **UNRUN** |
| `cargo clippy --workspace --all-targets -- -D warnings` | existing `static-checks` job (UNCHANGED) | Yes — workspace-wide | **UNRUN** |
| `cargo test --workspace` | existing `unit-tests` + `integration-tests` jobs (UNCHANGED) | Yes — workspace-wide | **UNRUN** |
| Plugin conformance suite as its own CI job | NEW `plugin-sdk-conformance` job: `cargo test -p flexcore-kernel --test conformance_p7_21 -- --nocapture` + `cargo test -p flexcore-plugin-host --test untrusted_plugin_p7_23` + the P7.26 perf harness + the P7.35 lexical-scan test; `suite-id=flexcore-plugin-sdk-conformance-suite-v1` and the wit world pins (`flexcore:optimizer@1.0.0 flexcore:grid@1.0.0 flexcore:telemetry@1.0.0`) logged in the job lines (the P6.27 step-2 pattern) | Yes | **UNRUN — CI execution UNRUN (human gate input); never claimed green** |
| Plugin manifest validation | NEW `plugin-manifest-validation` job: `cargo test -p flexcore-kernel --test manifest_p7_05` — the P7.05 kernel validator IS the gate; no separate tool invented (recorded decision) | Yes | **UNRUN** |
| WIT compatibility validation | NEW `wit-validate` job: the pin-record consistency check (P7.40: `wasm-tools $WASM_TOOLS_VERSION` must appear in `wit/TOOLCHAIN.md` §1 — a tool upgrade requires the same-PR pin-record edit), install at the recorded pin, then `wasm-tools component wit wit/` (the PREPARED-UNRUN command, `wit/TOOLCHAIN.md` §4) | Yes (`wit/` packages + the three worlds) | **UNRUN — config only** |
| Buf lint + breaking (worker schemas) | NEW `worker-contract-buf` job over the NEW `python/workers/contracts/` buf module (`worker_api.proto`); lint always, breaking on PR against the base ref — mirroring the P1.18 `proto/` governance; the existing proto/ buf steps are UNCHANGED | Yes | **UNRUN — config only** |
| SBOM generation | existing CycloneDX step in `security-and-supply-chain` (`cargo cyclonedx --all`) + the existing Syft artifact in `edge-supply-chain` (UNCHANGED) — workspace-wide, so the SDK crates are covered by the same runs | Yes | **UNRUN** |
| Container/dependency scanning | existing `edge-supply-chain` (cargo-audit, Syft) (UNCHANGED) — lockfile-scoped, so the SDK crates' transitive set is covered | Yes | **UNRUN** |
| License/security review | existing `edge-license-gate` (UNCHANGED) — `cargo deny check licenses` backed by `deny.toml` `unlicensed = "deny"`; the allowlist already covers the SDK stack (§5) | Yes | **UNRUN** |
| Scenario regression suite | existing `scenario-regression` / `phase3-scenario-regression` (UNCHANGED); the SDK's own scenario suite is P7.43's deliverable (open, owned — not claimed here) | Platform layer | **UNRUN** |
| Python Ruff + type checks + pytest | **RECORDED SEAM — NOT WIRED**: `python/workers/requirements.txt` carries deliberately un-installable `==UNPINNED-REVERIFY` pins, so an install step would fail by construction until the human pins verified versions (its recorded re-verify procedure). Wiring that job is the activation wave's step. Also recorded in the ci.yml header comment | n/a until wired | **NOT WIRED (by decision, with reason)** |

Path-filter decision (the P6.27 D-3 precedent, repeated): NO path filters
were added to any existing job, and no existing trigger was touched —
adding filters would RESTRICT existing coverage (non-additive).

### 2.2 Green-run record: UNRUN — human gate input

Every job in the matrix above is recorded **WRITTEN-UNRUN**. The green CI
run on the phase branch is the P7.27 verification point and is NEVER
claimed here.

### 2.3 The "unlicensed dependency fails CI by construction" demonstration

Configured via `deny.toml` `unlicensed = "deny"` + the existing FAILING
gate job `edge-license-gate`. The scratch-branch demonstration (introduce
a test-unlicensed dependency, confirm CI fails, remove it) is a **HUMAN
GATE INPUT — recorded UNRUN** (the P6.27 §2.3 honesty pattern, unchanged).

---

## 3. P7.35 — the ADR-006 no-dylib gate

### 3.1 The lexical gate (CI job + test twin)

- CI: NEW `no-dylib-gate` job in `ci.yml` — grep-based over every member
  source tree (`crates`, `apps`, filtered to `/src/`), the plugin/example
  trees (`plugins/`), and every workspace `Cargo.toml`; fails the build on
  ANY hit. Explicit allowlist MECHANISM with zero entries: any entry
  requires an exact path + a dated review comment in the same PR (no
  silent allowlist entries — the ledger's named risk).
- Test twin: NEW `apps/plugin-host/tests/no_dylib_p7_35.rs` — the SAME
  calibrated tokens in the `no_inbound_listener.rs` structural-scan style:
  one test over `apps/plugin-host/src/**` (≥ 8 files), one workspace-wide
  twin over `crates/*/src` + `apps/*/src` + all `Cargo.toml` + `plugins/`
  (≥ 40 src files, ≥ 15 manifests), and one defence-in-depth pin asserting
  the kernel manifest validator's negative `class: "dylib"` rejection case
  stays in place.

### 3.2 Token calibration — run BEFORE the gate was written; the recorded baselines

The gate is green-by-construction: the exact commands and their ACTUAL
outputs (run 2026-11-13, Git Bash on win32 x64) are quoted here verbatim.

Baseline 1 — loader tokens over ALL `.rs` (whole repo, `crates apps plugins`):

```text
$ grep -rln -E "libloading|dlopen|#\[link\(name|extern \"C\"|LoadLibrary|GetProcAddress" --include="*.rs" crates apps plugins 2>/dev/null; echo "exit=$?"
exit=1
```

(Zero files — grep exit 1 = no matches, no output.)

Baseline 2 — manifest tokens + `crate-type` over ALL `Cargo.toml`:

```text
$ grep -rn -E "libloading|dlopen|LoadLibrary|GetProcAddress|crate-type" --include="Cargo.toml" . 2>/dev/null | grep -v "/target/"; echo "exit=$?"
exit=1
$ grep -rn -E "\bdylib\b|cdylib" --include="Cargo.toml" . 2>/dev/null | grep -v "/target/"; echo "exit=$?"
exit=1
```

Baseline 3 — the bare `dylib` word (calibration; NOT a gate token):

```text
$ grep -rn -E "dylib|dlopen|libloading|#\[link\(name|extern \"C\"" --include="*.rs" crates/*/src apps/*/src plugins 2>/dev/null; echo "exit=$?"
crates/edge-agent/src\adapters\registry.rs:8:// not dylibs and not runtime reflection).
exit=0
$ grep -rln -E "dylib|dlopen|libloading|#\[link\(name|extern \"C\"" --include="*.rs" crates apps plugins 2>/dev/null; echo "exit=$?"
crates\edge-agent\src\adapters\registry.rs
crates\flexcore-kernel\tests\manifest_p7_05.rs
exit=0
```

The two bare-word hits are BOTH benign defences: (1) the edge registry's
prose comment explaining WHY mapping tables are used ("not dylibs and not
runtime reflection"); (2) the kernel manifest validator's NEGATIVE test
(`manifest_p7_05.rs` asserts a `class: "dylib"` manifest is REJECTED —
`UnknownClass`), which is part of the ADR-006 defence-in-depth. A bare-word
`.rs` token would false-fail on both; it is therefore deliberately NOT in
the token list. The dynamic-library crate-type shapes are caught by the
`Cargo.toml` scan (`crate-type` carrying `dylib`/`cdylib`).

### 3.3 The gate's documented limits (stated in the job comment AND here)

This is a LEXICAL gate. It catches direct dynamic-library loading patterns;
it cannot catch every possible FFI route (e.g. an indirect
build-script-generated binding, or a hand-rolled table spelled without any
listed token). The structural guarantee rests on THREE legs together:
(1) this gate, (2) the dependency rule
(`tools/check_dependency_rule.py` — the composition-crate graph), and
(3) review. ADR-006 remains a design decision that review enforces; the
gate makes the obvious routes mechanically fail. Test files are NOT
scanned (tests never load libraries at runtime; negative tests legitimately
name the tokens) — a documented exclusion, never a silent one.

### 3.4 Dependency-rule registration (verify-only, per the task)

`tools/check_dependency_rule.py` was checked, NOT edited: both crates were
already registered — `flexcore-kernel` (line 103, composition side) and
`flexcore-plugin-host` (line 111, composition side), with their crate dirs
listed (lines 129/133) among the 19 members. Nothing missed; no edit made.

### 3.5 Green run and the negative scratch-branch demonstration

UNRUN — human gate inputs (run the job; then add `libloading` on a scratch
branch and confirm BOTH the CI job and the test twin fail, then remove it).
Recorded, never claimed.

---

## 4. deny.toml — NO EDIT MADE (recording preferred)

Checked before writing anything: the `deny.toml` `[licenses]` allow list
ALREADY carries `"Apache-2.0 WITH LLVM-exception"` (the wasmtime family's
crates.io license expression) alongside plain `"Apache-2.0"`. The P7.03
audit (`03_sdk_license_audit.md` §7) already recorded the no-edit decision:
every SDK-stack license family maps to an entry already present
(LLVM-exception for the wasmtime family, MIT/Apache-2.0 for tonic/prost/
wit-bindgen legs, BSD-3-Clause for protobuf). **No additive entry was
structurally required, so `deny.toml` was not touched this wave.** If a
first lockfile-backed `cargo deny check` reveals a family not on the list,
the additive entry must carry the `# P7.03 — RE-VERIFY` comment style and
its own justification at that time; copyleft stays disallowed.

---

## 5. Decisions (DEFAULT-labelled where recommended, not decided)

| # | Decision | Status |
|---|---|---|
| D-1 | The P7.21 CI wiring lands as ONE job (`plugin-sdk-conformance`) carrying the conformance suite, the untrusted negatives, the perf harness, and the lexical-scan test — the P6.27 D-2 precedent (one suite/job rather than duplicate jobs re-running the same crates) | DEFAULT |
| D-2 | Plugin manifest validation is its own small job running the P7.05 kernel test target via cargo test; no separate validation tool is invented (the task's recorded instruction) | RECORDED constraint |
| D-3 | `wit-validate` pins only `wasm-tools` (1.239.0, ASSUMPTION-labelled) and couples the job to the `wit/TOOLCHAIN.md` §1 pin record via a grep check (the P7.40 §7 seam: "CI installs exactly the versions in wit/TOOLCHAIN.md §1"); `cargo-component`/`wit-bindgen` stay UNWIRED because no version was ever verified — installing an unpinned/unverified tool would violate AGENTS.md rule 1 and float the toolchain P7.40 forbids | DEFAULT |
| D-4 | The worker-contract buf module is a SEPARATE module at `python/workers/contracts/` (buf v2 module paths must live inside their module root, so it cannot be registered into `proto/`); the proto file's own header names "registration into the proto/ buf module (or a documented include path)" as the activation step — the separate-module form is that documented alternative | DEFAULT |
| D-5 | Python worker gates (Ruff/type-check/pytest) NOT wired: the requirements pins are deliberately un-installable strings; wiring a job that fails at install by construction adds noise, not a gate. Recorded as a seam with owner (activation wave) | DEFAULT (honesty) |
| D-6 | The perf harness writes NO `generated-bindings` code arm: the gated implementation belongs to the bindings-build owner, and enabling the feature without it is the crate's recorded loud compile error (fail-closed by design) | DEFAULT |
| D-7 | Token calibration BEFORE the gate (the task's ordering rule): the greps in §3.2 were executed and their outputs recorded before the token lists were finalised, so the gate is green-by-construction | RECORDED constraint |

## 6. Human gate inputs (all with EMPTY results until executed)

| # | Gate input | How | Result |
|---|---|---|---|
| 1 | Green CI run on the phase branch (all jobs incl. the five new ones) | push/PR; the run record is the P7.27 evidence | (EMPTY — UNRUN, never claimed) |
| 2 | Unlicensed-dependency demonstration on a scratch branch | introduce a test-unlicensed dependency, confirm `edge-license-gate` fails, remove it | (EMPTY — UNRUN) |
| 3 | no-dylib negative demonstration on a scratch branch | add `libloading` to a member crate, confirm `no-dylib-gate` AND `no_dylib_p7_35` fail, remove it | (EMPTY — UNRUN) |
| 4 | First `wasm-tools component wit wit/` verdicts + release-page re-check of the 1.239.0 ASSUMPTION | run `wit-validate` / the prepared steps in `wit/TOOLCHAIN.md` §4 | (EMPTY — UNRUN) |
| 5 | Fill the P7.26 measured tables (both profiles, external CPU/memory observation, the pilot load profile) | §1.3 filling commands on the target hardware | (EMPTY — not measured) |
| 6 | `buf lint` / `buf breaking` first verdicts on `worker_api.proto` | the `worker-contract-buf` job (or locally with a buf CLI at a recorded version) | (EMPTY — UNRUN) |
| 7 | Wire the Python worker gates after the human pins verified requirements versions | activation wave | (EMPTY — seam open) |

## 7. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in §8) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in §8) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (output quoted in §8) |
| Baseline lexical greps (§3.2) | RUN in the writing loop (outputs quoted verbatim) — file reads only |
| `cargo build` / `cargo test` / `cargo clippy` (any target) | **NOT RUN — banned in this loop; human gate inputs 1–3** |
| CI execution (any job) | **UNRUN — human gate input 1** |
| buf / wasm-tools / cargo-component / cargo-deny / cargo-audit / syft execution | **NOT RUN — banned in this loop** |
| Performance measurements | **NONE RECORDED — tables deliberately EMPTY; no number invented** |

## 8. Permitted-tool output

The three permitted tools were run after every file in this wave was
written; their ACTUAL output is quoted in the agent completion report and
summarized in §7.

## 9. Related records

- `19_20_36_config_health_path.md`, `10_11_plugin_host_broker.md`,
  `12_13_enforcement_negatives.md` — the host seams the perf harness drives.
- `21_22_23_conformance_swap_untrusted.md` §2.4 — the conformance-suite CI
  seam this wave wires (suite id + the exact cargo test targets).
- `40_toolchain_pinning.md` §7 — the CI pin-check seam `wit-validate` implements.
- `03_sdk_license_audit.md` §7 — the deny.toml no-edit decision this wave
  verified and reused.
- `ledger/phase-06-edge-protocol/26_27_28_docs_ci_drill.md` — the CI gate
  matrix + EMPTY-run-record honesty pattern this record follows.
- `apps/worker/tests/supervision_p7_18.rs` — the injected-clock supervisor
  determinism referenced (not duplicated) by the worker-side rows.
