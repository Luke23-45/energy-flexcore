# P7.41 + P7.42 + P7.43 — Manifest-validator fuzz/property tests, plugin-path determinism, and the SDK scenario-regression suite

Date: 2026-11-13. Subtasks: P7.41, P7.42, P7.43 (`ledger_phase_7.md`
§P7.41–§P7.43). Owner role: Rust engineer (SDK) + DevOps. Report trace:
§19.2(2)(5)(6); §5.5; §5.7; §24 step 10; §22.2; §23 Stage 7. Inputs:
P7.05/P7.06 (`05_06_manifest_resolution.md`); P7.17/P7.18
(`17_18_workers_supervision.md`); the P7.05/P7.07/P7.13/P7.16/P7.19/
P7.23/P7.36 suites; P7.22/P7.27; the P5.19/P5.20/P6.27 reproducibility and
scenario-regression patterns; P4.31 (`31_fault_shadow.md`); the P6.28
drill-report EMPTY-cell pattern
(`ledger/phase-06-edge-protocol/26_27_28_docs_ci_drill.md` §3).

## 0. Honest status — READ FIRST

**Everything in this wave is WRITTEN, NEVER RUN.** No cargo command
(build/test/clippy/fmt) was executed; no fuzzer ran; no scenario executed;
no CI run happened. Every test suite below is written against the public
surfaces of the existing crates (grep-read, not compiled) and is expected
to compile and pass at the first human build pass. The permitted
verification tools (`tools/verify_docs.py`, `tools/validate_agent.py`,
`tools/check_dependency_rule.py`) ran green at delivery (§6); a Rust test
passing is NOT claimed anywhere. No safety, certification, or
protection-grade claim is made; the plugin path remains
simulation/shadow-only (§21.4).

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `crates/flexcore-kernel/tests/fuzz_manifest_p7_41.rs` | NEW (P7.41) |
| `crates/flexcore-kernel/tests/determinism_p7_42.rs` | NEW (P7.42 kernel leg) |
| `apps/plugin-host/tests/determinism_p7_42.rs` | NEW (P7.42 host leg) |
| `plugins/scenarios/sdk-01-manifest-to-plan.json` | NEW (P7.43) |
| `plugins/scenarios/sdk-02-profile-switch-with-drain.json` | NEW (P7.43) |
| `plugins/scenarios/sdk-03-config-drift-refusal.json` | NEW (P7.43) |
| `plugins/scenarios/sdk-04-proposal-envelope-replay-refused.json` | NEW (P7.43) |
| `plugins/scenarios/README.md` | NEW (P7.43 format doc) |
| `crates/flexcore-kernel/tests/scenario_regression_p7_43.rs` | NEW (P7.43 interpreter) |
| `.github/workflows/ci.yml` | EDITED (additive: one header comment block + one NEW job `sdk-scenario-regression`; NO existing job or step rewritten) |
| `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | EDITED (P7.30/P7.41/P7.42/P7.43 ticks ONLY) |
| `ledger/phase-07-plugin-sdk/30_failure_drill.md` | NEW (P7.30's own record — procedure + EMPTY report) |
| this file | NEW (P7.41/P7.42/P7.43 decision record) |

No crate `src/` file was touched (P7.41/P7.42/P7.43 are test-and-config
subtasks; the kernel and host sources are read-only here). No `Cargo.toml`
was touched (no new dependency — the fuzz generator is a hand-rolled seeded
LCG, not proptest/quickcheck; the scenario interpreter uses the kernel's
existing `serde_json` pin).

## 1. Per-subtask coverage

| Subtask step | Delivered | Where | Status |
|---|---|---|---|
| P7.41 steps 1–3 (property tests, fuzz, SemVer-range property) | The dependency-free fuzz/property harness: a seeded deterministic LCG generator (fixed seed `0x464C_5843_5037_3431`, "FLXP741" — the P7.18 seed naming pattern) producing mutated manifests (truncation, key injection, type flips, deep nesting, unicode/control keys, huge strings, oversize documents, targeted semantic mutants) plus a 26-entry fixed adversarial corpus, under properties P1 (never panics — `catch_unwind`), P2 (every rejection is a named error from a re-declared name table), P3 (validity stable — every input parsed twice, verdicts equal), P4 (valid fixtures remain valid across every generator pass), the ledger step-1 targeted mutants (missing required field, malformed SemVer, malformed range, unknown class, undeclared/never-granted permission — never validate), and the step-3 range property (generated comparator expressions checked against an independent reference matcher over a dense version grid, PLUS crafted cases where lexicographic and numeric ordering disagree — the numeric answer must win) | `crates/flexcore-kernel/tests/fuzz_manifest_p7_41.rs` | Written-unrun |
| P7.41 step 2 (fuzz for a recorded duration; never crash or hang) | The in-repo fuzz IS the deterministic property harness above (bounded inputs, fixed iteration counts — no hang is constructible: the validator's loops are all bounded by the size-capped input). The REAL libFuzzer target is RECORDED, NOT ADDED (§2 D-1) | same | Written-unrun; libFuzzer target recorded |
| P7.42 steps 1–2 (defined replay; same inputs → same schedules) | The kernel-side determinism proof: envelope construction is a pure function (build-twice byte-identical canonical string + digest, asserted on EVERY scenario build too); the full sequence request-envelope build → path-protection checks → proposal envelope is identical across two independent runs (fresh checkpoints); replaying a recorded sequence after a checkpoint restore yields identical outcomes and identical refusal names; window decisions are injected-clock-only and refusals never advance the window; the P7.18 backoff schedule mirrored (read-only citation; §3.2); resolver/profile/config outputs pure. The host-side half: enforcement verdicts, broker grant derivation and checks, health snapshots, and limits conversions are pure functions with identical audit trails (the gate/broker/monitor read NO clock — every timestamp is a caller parameter) | `crates/flexcore-kernel/tests/determinism_p7_42.rs`, `apps/plugin-host/tests/determinism_p7_42.rs` | Written-unrun |
| P7.42 step 3 (nondeterminism sources listed, excluded from the claim) | Recorded: fuel-limit behaviour, Wasmtime-version sensitivity, and worker timing are LIVE-RUN-only sources and are EXCLUDED from every equivalence claim in both test files (stated in the headers); the wall-clock residual and the §24-step-10 live replay through a running host+worker are the recorded human/deployment follow-up (§3.4 D-5) | this record §3.3; the two test-file headers | Recorded |
| P7.42 step 4 (replay record filed with the P7.28 evidence pack) | The test files and this record ARE the unit-level replay record; the LIVE replay record (both runs, comparison, pre-recorded tolerance, nondeterminism notes) is filed by the human build pass into the P7.28 evidence pack per the recorded procedure | this record §3.4 | Recorded follow-up |
| P7.43 steps 1–2 (named scenarios with pinned inputs/seeds/outcomes; versioned artifact) | Four named SDK scenarios as pinned-input JSON data under `plugins/scenarios/` (format documented in `plugins/scenarios/README.md`), each carrying `scenario-id`, `scenario-version`, the suite id, `pins.{seeds,versions,tolerance}`, embedded inputs, and per-step expected outcomes as NAMED verdicts; the suite id `flexcore-plugin-sdk-scenario-regression-v1` is asserted per scenario and printed per run | `plugins/scenarios/*.json`, `plugins/scenarios/README.md` | Written-unrun |
| P7.43 step 3 (wire into CI; failure blocks the merge) | NEW additive CI job `sdk-scenario-regression` running `cargo test -p flexcore-kernel --test scenario_regression_p7_43` with the suite-version log step, in the existing job style | `.github/workflows/ci.yml` | Config only — UNRUN |
| P7.43 step 4 (first green run with suite version) | NOT DONE — a green run is a human gate input; never claimed | — | UNRUN |

## 2. P7.41 — what was implemented, and the recorded findings

### 2.1 The generator's determinism

The generator is a 64-bit LCG (Knuth's MMIX constants) seeded with the
recorded constant `GENERATOR_SEED = 0x464C_5843_5037_3431` ("FLXP741").
The same seed produces the same input sequence on every run on every
platform — this is what makes the ledger's verification method
("re-run the fuzz suite from the recorded seed and compare the outcome
class counts") executable: the human re-run must produce EXACTLY the same
input set and the same verdict classes. The suite runs 3 generator passes
of 512 mutations each (seeds `GENERATOR_SEED`, `+1`, `+2`) plus the fixed
corpus, so the re-run comparison is over 1 536 generated inputs + 26
corpus entries.

Mutation classes: truncation at a char-boundary offset, key injection
(including unicode/zero-width/long keys), type flips on every declared
top-level field, deep nesting (100–200 levels — past serde_json's
documented 128-level recursion limit, surfacing as a clean parse error),
unicode and control characters inside declared string fields, huge strings
(over the per-field caps), size-gate oversize documents (padded past the
64 KiB `MAX_MANIFEST_BYTES`), and the targeted semantic mutants.

The targeted step-1 property uses DETERMINISTIC mutants (built from the
valid fixture by exact string replacement, not generator draws), so the
"never validates" property does not depend on random coverage: missing
`plugin-id`/`api-version`, malformed SemVer (`1.0` and `1.0.0-beta+build`),
malformed range (`>=x.y.z`), unknown class (`kernel-mode`), undeclared
permission (`not.a.permission`), never-granted permission
(`dispatch.issue`).

### 2.2 The properties (and what each one pins)

| Property | Assertion | What it rules out |
|---|---|---|
| P1 | `catch_unwind` around every parse; a panic names the input | the ledger risk "a validator that rejects malformed input in the lab but panics on it in the host" |
| P2 | every `Err` has `name()` in a re-declared 24-name table; an unnamed or drifted name fails the suite | a silent rejection-class change |
| P3 | every input parsed twice; `Valid(Manifest)` payloads equal (`PartialEq`) and refusal names equal | nondeterministic validation (map-order dependence, etc.) |
| P4 | the two valid fixtures re-parse `Ok` after every full generator pass | a mutation colliding into a fixture-corrupting false-acceptance |
| step-1 | targeted mutants NEVER validate | required-field/SemVer/class/permission holes |
| step-3a | 128 generated comparator expressions over a dense version grid vs an independent reference matcher | a string-compare or semantic drift in range decisions |
| step-3b | six crafted cases where lexicographic and numeric ordering DISAGREE (`>=0.10.0` vs `0.9.0`, `~0.5.2` vs `0.5.11`, …) — the numeric answer must win | string-comparison sneaking into range decisions (the P7.05 acceptance rule) |
| step-3c | 15 malformed range fragments refused alone AND after a valid prefix, while the prefix alone is accepted | a string-acceptance of malformed fragments |

The config-surface twin (P7.19's `resolve_for_instance`) runs the same
P1/P2/P3 discipline over its own recorded 18-name error table (the ledger
names the config validators among the fuzz targets too).

### 2.3 The serde_json duplicate-key finding path (recorded limitation, pinned honestly)

The recorded limitation (`05_06_manifest_resolution.md` §3 item 3):
`serde_json::Value` maps silently keep the LAST occurrence of a duplicated
JSON key; rejecting duplicate keys needs a custom parser and is NOT
implemented. The P7.41 tests PROVE the limitation and pin the honest
claim:

1. `duplicate_keys_the_recorded_serde_json_last_wins_collapse_is_proven`
   proves the collapse at the parser layer (`{"a": 1, "a": 2}` → one key
   with value 2).
2. `duplicate_keys_verdict_is_refused_or_deterministic_here_deterministic_last_wins`
   pins the acceptance-class outcome for a constructed duplicate
   `plugin-id` manifest: the validator CANNOT detect the duplicate, so the
   pinned claim is **DETERMINISTIC-LAST-WINS** — the verdict on the
   duplicate-key input equals the verdict on its collapsed form, stable
   across runs, and the parsed manifest carries the LAST declared plugin
   id. This satisfies the recorded acceptance rule
   ("REFUSED-or-DETERMINISTIC") on its DETERMINISTIC half; it does NOT
   claim detection. If a future kernel adds duplicate detection, the test
   FAILS LOUDLY (its `Refused` branch panics with a re-record instruction)
   — the pin flips by a reviewed change, never silently.

### 2.4 Recorded decisions (P7.41)

| # | Decision | Status |
|---|---|---|
| D-1 | The real libFuzzer target (`cargo-fuzz`, `fuzz_target!` over `parse_manifest`, including a non-UTF-8 byte corpus class) is a RECORDED human/deployment follow-up, NOT added: it requires the `cargo-fuzz` tooling and a `fuzz/` crate surface (new `Cargo.toml` entries), which this loop must not add. The non-UTF-8 class is structurally unreachable through the current `&str` entry point (Rust guarantees `str` is valid UTF-8) — the corpus covers replacement/combining/RTL/zero-width/control characters instead | RECORDED |
| D-2 | No proptest/quickcheck — the generator is a hand-rolled seeded LCG; `flexcore-kernel/Cargo.toml` is untouched (dependency-minimisation rule) | DEFAULT |
| D-3 | Duplicate keys are pinned as DETERMINISTIC-LAST-WINS (the recorded limitation), not as refused — see §2.3 | RECORDED |
| D-4 | The name tables (24 manifest error names; 18 config error names) are RE-DECLARED in the test so a silent error-name change fails P2 | DEFAULT |

## 3. P7.42 — what is proven where, and what is excluded

### 3.1 The proof split (kernel leg / host leg)

The kernel cannot depend on the worker crate and the host cannot depend on
the kernel's test fixtures, so the proof is split by crate boundary and
each half asserts the pure-function properties of ITS crate's surface:

- **Kernel leg** (`crates/flexcore-kernel/tests/determinism_p7_42.rs`):
  envelope construction purity; the FULL sequence (envelope build → four
  path checks → response echo → proposal record) identical across two
  independent runs on fresh checkpoint files; replay after checkpoint
  restore yields `SEQUENCE_REPLAYED` / `SEQUENCE_OUT_OF_ORDER` with the
  first-run refusal names and identical refusal objects; window decisions
  (`REQUEST_NOT_YET_VALID` / `REQUEST_EXPIRED`) are injected-clock decisions
  and refusals never advance the window; lifecycle audit timestamps equal
  injected clock readings exactly; resolver / profile→plan / config
  outputs are pure functions (equality across runs, byte-identical JSON).
- **Host leg** (`apps/plugin-host/tests/determinism_p7_42.rs`): enforcement
  verdicts are pure (identical `EnforcementVerdict` objects from two
  independently composed gates — including the broker's deterministic
  `grant/<plugin>/<capability>/<seq>` ids — and identical refusal objects
  with their names, `PERMISSION_ABOVE_PROFILE` / `LIMITS_INVALID` /
  `PERMISSION_VOCABULARY_DRIFT`); broker grant derivation and checks are
  pure (identical receipts, identical denials, expiry decided purely by
  `(grant, now)`); health snapshots deterministic for identical event
  sequences (equal snapshots, byte-identical JSON, order independent of
  registration order, equal audit trails); limits conversions exact
  (16 pages/MiB, exact byte conversions, round-up epoch ticks, recorded
  defaults, zero-fuel refusal).

### 3.2 The P7.18 backoff mirror (read-only citation, recorded honestly)

`flexcore-kernel` cannot depend on `flexcore-worker` (the kernel is
upstream of every consumer; `tools/check_dependency_rule.py` classifies it
COMPOSITION with nothing depending on it). The kernel-side assertion is
therefore a MIRRORED recomputation of the recorded formula and constants
(`apps/worker/src/supervision.rs::RetryScheduler`: seed
`0x464C_5843_5037_3138` "FLXP718", base 250 ms, cap 5 000 ms, jitter cap
250 ms, xorshift64 jitter, `delay = min(base·2^attempt, cap) + jitter`) and
pins: the pure-function property across attempts, the cap bound, the
jitter bound, and the recorded low-attempt delay windows. The DIRECT
assertion against the real `RetryScheduler` lives in
`apps/worker/tests/supervision_p7_18.rs`; if the two ever disagree, the
worker suite wins and the mirror is re-recorded. Stated in the test header
verbatim.

### 3.3 Nondeterminism sources — listed, and excluded from the claim

| Source | Treatment |
|---|---|
| Fuel-limit behaviour (metered-instruction consumption varies with wasmtime version/host) | LIVE-RUN only — excluded from every equivalence claim in the test files; dispositioned at the live §24-step-10 replay |
| Wasmtime-version sensitivity | LIVE-RUN only — the replay pins the wasmtime version per P7.40 before comparing runs |
| Worker timing (gRPC scheduling, probe cadence) | LIVE-RUN only — excluded; the supervisor's SCHEDULE is deterministic (seeded backoff), its TIMING is not claimed |
| Wall-clock anywhere in the outputs | Asserted structurally per crate: every kernel time-consuming type is constructed with an explicit `&dyn Clock`; every host timestamp is a caller-supplied parameter (no parameterless time-taking call exists on the tested surface). The cross-crate grep (`SystemTime::now`/`Instant::now` over the touched module paths) is not possible from inside the tests — recorded as a step of the human build pass (D-5) |

### 3.4 Recorded decisions (P7.42)

| # | Decision | Status |
|---|---|---|
| D-5 | The FULL §24-step-10 replay through a LIVE host + worker (one WASM component, one gRPC worker, pinned FlexCore/wasmtime/WIT/worker/plugin versions, seeds recorded, both runs compared within a tolerance recorded BEFORE the second run, record filed with the P7.28 evidence pack) is a HUMAN/DEPLOYMENT follow-up — the loop bans forbid running anything | RECORDED |
| D-6 | The tolerance for the LIVE replay must be pre-recorded (the ledger risk: "a tolerance fitted after seeing the second run's difference"); the unit-level assertions here need none (exact equality) | RECORDED |

## 4. P7.43 — the scenario suite, its version, and the CI wiring

### 4.1 The format and the suite version

The scenario format is documented in `plugins/scenarios/README.md` (a
closed step vocabulary over the PUBLIC kernel APIs: `parse-manifest`,
`resolve`, `parse-profile`, `validate-profile`, `diff-plans`,
`execute-switch`, the lifecycle steps, `resolve-config`, the plugin-path
steps, `advance-clock`). Every scenario pins: inputs (embedded manifests /
profiles / configs), `pins.seeds` (each scenario records its seeds
position — for the structural kernel paths it is "none", stated, never
assumed), `pins.versions`, `pins.tolerance` (exact — named refusals and
exact structural equalities; no numeric tolerance exists on these paths),
and per-step expected outcomes. The suite id
`flexcore-plugin-sdk-scenario-regression-v1` is asserted per scenario by
the interpreter and logged per CI run (P7.43 step 2). Scenario changes are
build-visible (the files are `include_str!`-embedded), so a suite that
"grows silently between runs" (the recorded risk) cannot happen without a
source change.

### 4.2 The CI job (additive)

One new job `sdk-scenario-regression` in `.github/workflows/ci.yml`,
inserted after `plugin-sdk-conformance`: toolchain install → suite-version
log step → `cargo test -p flexcore-kernel --test scenario_regression_p7_43
-- --nocapture`. NO existing job or step was rewritten. A scenario failure
fails the job and blocks the merge (P7.43 step 3). The green run is UNRUN
(P7.43 step 4 is a human gate input; the ledger tick carries
"awaiting human build/test").

### 4.3 Cross-reference (recorded, no duplication)

This suite is the SDK leg of the platform scenario-regression suite
pattern:

| Leg | Where | Relationship |
|---|---|---|
| Platform (P5.20) | `scenarios/library/` + the `scenario-regression` CI job (sim-runner run/replay verdict) | Pattern source; P7.43 reuses the named-scenario + pinned-expectation + CI-blocking pattern; no scenario is shared or duplicated |
| Edge (P6.27/P6.28) | the `edge-agent-suite` CI job + the P6.28 drill | The drill-report EMPTY-cell pattern is reused by P7.30 (`30_failure_drill.md` §3); the suite pattern by P7.43 |
| SDK (P7.43) | `plugins/scenarios/` + the `sdk-scenario-regression` CI job (this wave) | Composition/lifecycle/config/path-protection kernel scenarios; execution legs stay with P7.21/P7.22/P7.28 |

### 4.4 The interpreter's internal determinism pin

Every `build-envelope` step asserts (inside the interpreter, for every
scenario) that constructing the envelope twice yields byte-identical
canonical strings and digests — the P7.42 envelope-purity property is thus
also exercised on every scenario run, not only in the dedicated suite.

## 5. The recorded follow-ups for the coordinator

| # | Follow-up | Trigger / owner |
|---|---|---|
| F-1 | **P7.38/P7.39 extension scenarios** (the planned `sdk-05-revoke-then-verify-refused`, plus tenant-scope scenarios once the APIs exist) are NOT written: at authoring time the concurrent P7.38/P7.39 files (`crates/flexcore-kernel/src/revocation.rs`, `tenant_scope.rs`, the host `tenant_scope` additions, and their test files) were CHECKED and are NOT on disk. Writing scenarios against a PLANNED API shape is forbidden; they are added after those subtasks land, against the real API surfaces and their decision record | Coordinator, after P7.38/P7.39 close |
| F-2 | The libFuzzer `cargo-fuzz` target over `parse_manifest` (and the config/profile parsers), with a recorded seed/duration and outcome-class counts compared on re-run (D-1) | Human/deployment build pass |
| F-3 | The LIVE §24-step-10 plugin-path replay record filed with the P7.28 evidence pack (D-5/D-6) | Human/deployment build pass; P7.28 owner witnesses |
| F-4 | The wall-clock grep step (`SystemTime::now` / `Instant::now` over `crates/flexcore-kernel/src`, the plugin path, and the host modules touched by P7.42) recorded with its (expected-empty) output | Human build pass |
| F-5 | The `sdk-scenario-regression` CI job's first green run recorded with the suite version (P7.43 step 4), plus the scratch-branch break-demonstration (the ledger verification method: one scenario's expected outcome flipped → CI fails) | Human gate input |

## 6. Verification (the loop's permitted tools; actual output)

Permitted tools only; no cargo command of any kind was executed.

```text
$ python -I tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC

$ python -I tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid

$ python -I tools/check_dependency_rule.py
OK — dependency rule holds for 19 workspace members.
```

All three exited 0 at delivery. Everything else — the Rust suites, the
fuzz harness, the scenario suite, the CI job — is WRITTEN-UNRUN and awaits
the human build/test pass. The ledger ticks read "implemented, awaiting
human build/test" and carry this record's path.
