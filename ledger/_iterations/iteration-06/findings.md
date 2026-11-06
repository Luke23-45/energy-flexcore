# Iteration 6 — Testing / Verification / Reproducibility Review: Findings

| Field | Value |
|---|---|
| Iteration | 06 — testing, verification, reproducibility |
| Scope | `ledger/phase-07-plugin-sdk/`, `ledger/phase-08-advanced-coordination/`, `ledger/phase-09-pilot-qualification/` (read-only for all other phases) |
| Reviewer role | Iterations 5 and 6 reviewer (independent; no favour to the draft) |
| Date | 7 November 2026 |
| Baseline counts (before Iteration 6 patching, after Iteration 5) | P7: 39 subtasks; P8: 36; P9: 36 (P7.33–P7.39, P8.31–P8.36, P9.31–P9.36 added by Iteration 5) |
| Baseline counts (original, before either iteration) | P7: 32; P8: 30; P9: 30 |

## Method — what was re-read and how

Re-read in full, against the ledger text:

- Report §19 (simulation strategy, test layers §19.2(1)–(10), benchmark cases
  §19.3, evaluation metrics §19.4), §22.2 (CI gates), §24 step 10 (replay the
  same scenario with the same versions and verify reproducibility), §23
  Stage 7/8/9 exit gates (verbatim), §13.1–§13.3 (research evaluation
  boundaries), §9.2 (result/provenance requirements).
- `ledger_phase_7.md` (P7.01–P7.39 incl. Iteration 5 additions),
  `ledger_phase_8.md` (P8.01–P8.36), `ledger_phase_9.md` (P9.01–P9.36),
  each phase's `CHECKLIST.md` and `RESEARCH.md`.
- Coverage checks run by grep: `property test`, `contract test`, `scenario
  regression`, `deterministic`, `seed`, `replay`, `benchmark`, `HIL`,
  `fault injection`, `performance`, `load test`, `end-to-end`, `conformance`,
  `reproducib`, `tolerance`, `p95`/`p99`, `version pin`.
- Every §19.2 test layer was mapped per phase; every research module was
  checked against the Stage 8 exit gate ("every research module is compared
  to strong deterministic baselines and independently validated"); every
  P9 test subtask was checked for pre-set thresholds, frozen versions, and
  a defined re-run rule after a version change.

## Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 1 |
| Medium | 17 |
| Low | 7 |
| **Total** | **25** |

No Critical. The single High (I6-M8) is a pilot-acceptance hole: an adapter
could reach the field having never passed its named conformance suite in the
pilot configuration. The Medium items are mostly *unowned* test obligations:
the ledger names a gate or a suite (scenario regression, reproducibility
check, fault set) in one subtask but no subtask defines its contents, its
tolerance, or what happens when it fails.

---

## (a) Missing subtasks

| ID | Severity | Finding | Patches |
|---|---|---|---|
| I6-M1 | Medium | **P7 scenario-regression suite has no defined contents.** P7.27 lists "scenario regression suite" among the CI steps, but no subtask defines which scenarios, seeds, inputs, or expected outcomes it contains. P7.22 runs the §24 slice scenario for the swap test but never names it as a regression scenario, so the suite CI runs could be empty and still be "green". §22.2. | New **P7.43** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I6-M2 | Medium | **P7 toolchain versions are unpinned.** `wit-bindgen`, `cargo-component`, and `wasm-tools` versions are pinned nowhere in P7; P7.09's acceptance criteria never require recorded tool versions. P7 `RESEARCH.md` §7 task 2 explicitly lists this as an open verification task ("only `wasm-tools` 1.239.0 as of 26 August 2026 was found"). A WIT world that validates under one `wit-bindgen` and fails under another is unreproducible. | New **P7.40** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I6-M3 | Medium | **P7 manifest-validator testing is example-only.** P7.05 tests a fixed set of valid/invalid example manifests. There is no fuzzing or property testing of the manifest parser/validator — the component that stands between an untrusted plugin author and activation. A parser edge case (e.g. SemVer range parsing, unknown-field handling) found only by adversarial input would reach activation. §19.2(2). | New **P7.41** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I6-M4 | Medium | **§24 step 10 replay reproducibility is never applied to the plugin host.** P7.28's witness re-runs the exit-gate run book, but no subtask replays a scenario through the plugin host with the same versions/seeds and compares the proposed schedule against the earlier run within a stated tolerance. Fuel-limit determinism and Wasmtime-version sensitivity of component execution are unrecorded. §24 step 10, §19.2(5). | New **P7.42** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I6-M5 | Medium | **P8 has no unit- or property-test requirement for research code.** §19.2(1)–(2) require unit tests (units, sign conventions, solver I/O validation) and property tests (invariant preservation across generated states). P8 has failure-mode tests (P8.11/P8.14), a drill (P8.24), and CI pytest (P8.28) — but no subtask requires unit/property coverage of the research workers themselves. CI running pytest does not establish that the right tests exist. | New **P8.37** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I6-M6 | Medium | **P8 training-run reproducibility is untested and GPU nondeterminism is only a risk note.** P8.20's rerun test covers "a selected result" without distinguishing an *inference* rerun (reproducible) from a *training* rerun (may not be, especially on GPU). GPU nondeterminism is listed as a risk with no disclosure or tolerance criterion, so a training result could be presented as exactly reproducible when it is not. | New **P8.38** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I6-M7 | Medium | **P8 HELICS time-step mismatch and ANDES cross-tool consistency are untested.** P8.18's own risk states time-step mismatch "silently corrupting the interaction" — but no step tests detection of a mismatch. P8.16 runs ANDES power flow with no cross-check against pandapower on the same case, so a modelling error in the ANDES setup has no independent tripwire. | New **P8.39** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I6-M8 | High | **P9 adapter conformance suites are named but never executed in the pilot configuration.** P9.04 step 2 records "the conformance/interoperability test suite that applies" per adapter; P9.14 verifies telemetry end-to-end but no acceptance criterion requires *passing* the named suite in the pilot configuration. §2.3: "implementing an adapter is not a conformance claim" — yet the pilot could reach field acceptance with an adapter whose suite was never run. This is the one High in this iteration because it is the direct negation of a report rule at the point of field acceptance. | New **P9.37** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I6-M9 | Medium | **§24 step 10 replay is never applied at the pilot.** P9.14 records versions/seeds and P9.17 records versions/timestamps, but no subtask replays a recorded shadow period from stored inputs and compares the proposed actions against the originals within tolerance. The pilot therefore has no replay proof that its shadow results are reproducible — only that they were recorded. §24 step 10, §19.2(5). | New **P9.38** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I6-M10 | Medium | **P9 has no performance/load test at pilot scale.** §19.2(8) (telemetry ingest rate, scheduling runtime, query latency, command fan-out, recovery) has no P9 subtask. P7.26 measures the plugin host only, in the lab, on the §24 slice — not the pilot deployment at pilot scale. A pilot that passes functional acceptance and then falls over on ingest rate has not been tested for it. | New **P9.39** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I6-M11 | Medium | **P9 has no HIL→FAT traceability.** No subtask maps each field-acceptance case to the HIL/lab evidence that de-risks it, so a FAT case could rest on no upstream evidence and nothing would fail. P9 `RESEARCH.md` §4 states the validation pathway is digital-twin/simulation → HIL/PHIL → field "with the same disturbance profiles and seeds at each stage so evidence is comparable" — P9.15 never requires reusing the same profiles/seeds across stages. | New **P9.40** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I6-M12 | Medium | **P9 never re-runs the §22.2 scenario-regression suite against the pilot configuration.** The suite is a CI gate, but P9.14 re-verifies the Stage 6 exit gate (a different thing) and no subtask runs the regression suite on the pilot's adapters, grid model, and versions. A regression that only manifests in the pilot configuration would not be caught before field acceptance. | New **P9.41** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |

## (b) Issues / errors / contradictions

| ID | Severity | Finding | Patches |
|---|---|---|---|
| I6-E1 | Medium | **P8.20's rerun tolerance is undefined.** Acceptance: "at least one result per module passed the rerun test" and results reproduce "within stated tolerance" — but no step requires the tolerance to be recorded *before* the rerun. A tolerance chosen after seeing the rerun difference is not a test. | Amended **P8.20** in place (tolerance pre-recorded per module before the rerun) |
| I6-E2 | Medium | **P8.28's reproducibility check can pass against itself.** Acceptance says the seeded reproducibility check "passes in CI", while its own risk notes "a reproducibility check that passes because it compares against itself". No acceptance criterion requires the CI check to compare against the frozen baseline artifact (P8.06) rather than its own output. | Amended **P8.28** in place (CI check must compare against the frozen P8.06 artifact) |
| I6-E3 | Low | **P7.22's swap success criteria are unstated.** Steps record "results, timings" for baseline and swaps, but no equivalence criterion is declared in advance: same feasibility verdict? Same objective within what tolerance? Timing is inherently variable, so without a declared criterion a swap that changes the schedule could still be recorded as a pass. | Amended **P7.22** in place (pre-declared equivalence criteria) |
| I6-E4 | Low | **P8.23's spot-check has no blocking consequence.** Verification spot-checks three cells by re-running them, but no acceptance criterion states what happens if a spot-check fails — the comparison report could be published with a known-bad cell. | Amended **P8.23** in place (failed spot-check blocks the comparison report) |
| I6-E5 | Low | **P9.16 invalidates cases on version change but defines no re-run set.** Step 3 states "a version change during the run invalidates the affected cases" — but the plan never defines which cases must re-run after a change (affected subset vs full re-run), so the re-run scope would be decided after the fact. | Amended **P9.16** in place (re-run set defined in the plan) |
| I6-E6 | Medium | **P9's fault set omits plugin/worker crash.** §19.2(6) includes plugin crash; P9.14 step 4 runs a "fault-injection subset" (dropped/reordered/duplicated messages, stale telemetry, unavailable asset, solver timeout) and P9.20 drills degraded modes (central outage, link loss, asset unavailability, solver timeout, stale telemetry) — neither lists plugin-host or worker crash at the pilot, although the pilot runs plugins and workers. | Amended **P9.20** in place (plugin-host/worker crash added to the drill list) |
| I6-E7 | Medium | **Stage 8 exit-gate check: HELICS has no baseline comparison — defensible, but unrecorded.** Verified per module: ADMM ✓ (P8.09), GNN ✓ (P8.12), MARL ✓ (P8.15), dynamic evaluation ✓ (P8.17 vs deterministic baseline), topology-aware ✓ (P8.21), portfolio analytics ✓ (P8.22 vs baseline accounting). P8.18 (HELICS federation) has no baseline comparison, which is defensible — it is a coordinator, not a decision module — but no subtask *says* so, so a gate reviewer cannot tell a deliberate exclusion from an omission. | Amended **P8.18** in place (record why no baseline comparison applies, and what the comparison *is*: federation vs unfederated runs) |
| I6-E8 | Medium | **P8.19/P8.20 omit items their own steps imply.** P8.19 step 1 says "list the §19.3 base cases" without naming the IEEE 39/118-bus transmission cases explicitly, so they can be silently dropped. P8.20 step 1's reproducibility record lists benchmark version, solver/framework versions, model artifacts, seeds, configs, input snapshot IDs — but not dataset version or preprocessing version, although the training-data reference is the thing that determines what a GNN/MARL rerun actually trains on. | Amended **P8.19** (name 39/118 explicitly) and **P8.20** (dataset + preprocessing versions in the record) in place |
| I6-E9 | Low | **P9.15 has no HIL-platform calibration/validation record.** Step 2 records the platform's capabilities, time step, and interface method, but nothing requires evidence that the HIL rig itself is calibrated/validated — the evidence chain starts with an unvalidated instrument. | Amended **P9.15** in place (platform calibration/validation record required) |
| I6-E10 | Low | **P7.28's witness re-run does not specify a clean build/machine.** P7.24 requires a clean-machine walkthrough for the SDK docs; P7.28 step 3 has the witness re-run the run book "from the evidence pack alone" without stating clean build/machine, so a witness re-run on the author's machine could silently inherit state. | Amended **P7.28** in place (clean build/machine specified) |

## (c) Suggestions

| ID | Severity | Suggestion | Disposition |
|---|---|---|---|
| I6-S1 | Low | P7.26 measures broker overhead but sets no acceptance threshold or regression bound — a later change doubling broker latency would not fail anything. | Deferred — thresholds cannot be set honestly before the first measurement exists; P7.26's job is to produce the measured baseline, and the bound belongs in a follow-up once it does. Recorded here so it is not lost |
| I6-S2 | Low | P7.27 lists migration/schema tests among the CI gates; plugin-manifest schema migration (v1→v2) has no test subtask. | Deferred — the manifest is v1 only in this phase; migration testing belongs when a v2 schema is defined, not before. Recorded here |
| I6-S3 | Low | P8.12's headline metric (end-to-end feasible solve time) is correctly specified; suggest the P8.20 rerun test explicitly include one GNN held-out-topology re-run from artifacts, so the headline metric itself is the thing re-proven. | Patched: requirement written into new **P8.38** (rerun set must include one GNN held-out-topology inference rerun) |
| I6-S4 | Low | P9.17 shadow acceptance compares proposed actions against measurements; suggest the shadow-review record (P9.17 step 4) explicitly include tracking error and rejected-command reasons as review inputs, so the review cannot pass on pass-counts alone. | Not patched — P9.17 step 4 already requires the review to include "tracking error and any rejected commands with their reasons". Checked and already covered |

## (d) Stale research

| ID | Severity | Finding | Disposition |
|---|---|---|---|
| I6-D1 | Medium | **PyTorch/PyG version skew is unresolved, not stale.** PyTorch 2.15 (28 Oct 2026) is current; the newest PyG found, 2.8.0 (5 Jun 2026), supports PyTorch only up to 2.12 (P8 `RESEARCH.md` R8-C3). P8.03/P8.10 own the resolution (pin a verified-compatible pair or re-check for a newer PyG). This is a live constraint on the GNN work, not a ledger error. | No patch — owner exists (P8.03, P8.10); recorded so the GNN work does not pin "latest + latest" |
| I6-D2 | Low | **HELICS version current as of the cut-off was not verified from a primary source** (P8 `RESEARCH.md` §7 task 3). P8.18 step 2 already requires the HELICS version to be pinned and recorded at implementation time. | No patch — owner exists (P8.18) |
| I6-D3 | Low | **ANDES docs version vs release version.** Live ANDES docs show a development string (1.10.1.post50+) while PyPI's latest release is 2.0.0 (P8 `RESEARCH.md` R8-C5). P8.16 pins ANDES 2.0.0 (or a verified later release) and records which docs were consulted. | No patch — owner exists (P8.16) |
| I6-D4 | Low | **OpenADR 3.1 certification-profile status not verified** (P9 `RESEARCH.md` §5 open task). P9.04 must confirm before naming 3.1. The ledger already constrains this; no new subtask needed. | No patch — owner exists (P9.04) |
| I6-D5 | Low | **SBOM tool versions in P9 `RESEARCH.md` §6 are explicitly unverified** (Syft ≥1.30, Grype ≥0.100, Trivy v0.72.0 etc. "as reported by the cited sources, not verified … against the tools' own release pages"). The section itself forbids copying them into a release record without a check at P10.19 time. | No patch — correctly labelled as unverified background; owner is P10.19 (out of scope) |
| I6-D6 | Low | **SunSpec SVP / HIL platform choice is an open assumption, correctly labelled.** P9 `RESEARCH.md` §4 labels the HIL platform/lab/cost as an ASSUMPTION and assigns selection to P9.15. | No patch — owner exists (P9.15) |

## (e) Severity summary

- Critical: 0. High: 1 (I6-M8). Medium: 17 (I6-M1–M7, M9–M12, E1, E2, E6–E8).
  Low: 7 (I6-E3–E5, E9, E10, S1, S2 — plus D1–D6 watch items, of which D1 is
  Medium and D2–D6 are Low; see note).
- Note on counting: §(d) items are watch items, not defects; D1 is counted
  Medium because an unresolved PyTorch/PyG pin blocks GNN work, D2–D6 are
  Low. The defect total is 19 (12 missing + 7 issue items in §(a)–(b) that
  required patches, plus S3 patched into P8.38); S4 needed no patch, S1/S2
  are deferred.

## (f) Patch map — every item to an exact file + subtask ID

| Item | File | Subtask ID (new or amended) |
|---|---|---|
| I6-M1 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.43** |
| I6-M2 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.40** |
| I6-M3 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.41** |
| I6-M4 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.42** |
| I6-M5 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.37** |
| I6-M6 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.38** |
| I6-M7 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.39** |
| I6-M8 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.37** |
| I6-M9 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.38** |
| I6-M10 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.39** |
| I6-M11 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.40** |
| I6-M12 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.41** |
| I6-E1 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.20** |
| I6-E2 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.28** |
| I6-E3 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | Amended **P7.22** |
| I6-E4 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.23** |
| I6-E5 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | Amended **P9.16** |
| I6-E6 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | Amended **P9.20** |
| I6-E7 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.18** |
| I6-E8 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.19** + **P8.20** |
| I6-E9 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | Amended **P9.15** |
| I6-E10 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | Amended **P7.28** |
| I6-S1 / I6-S2 | — | Deferred (see §6) |
| I6-S3 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.38** |
| I6-S4 | — | No patch needed (P9.17 already requires it) |
| I6-D1–D6 | — | No patch — owners exist or correctly labelled |

## §6 — Applied vs deferred log

**Applied:** 4 new P7 subtasks (P7.40–P7.43), 3 new P8 subtasks (P8.37–P8.39),
5 new P9 subtasks (P9.37–P9.41), plus in-place amendments to P7.22, P7.28,
P8.18, P8.19, P8.20, P8.23, P8.28, P9.15, P9.16, P9.20, and the P9 CHECKLIST
(new items for P9.37–P9.41) and P7/P8 CHECKLIST additions for the
Iteration 5 + 6 gate-relevant subtasks. Subtask indexes, counts, and the
retrospective dependency ranges (P7.32, P8.30, P9.30) were updated to match.
Numbering continues the Iteration 5 sequence (P7.40+, P8.37+, P9.37+).

**Deferred (with reasons):**
- I6-S1 (broker-latency regression bound): no honest threshold exists before
  the first measurement; P7.26's measured baseline is the prerequisite.
- I6-S2 (manifest-schema migration tests): the manifest is v1 only in this
  phase; migration testing belongs when a v2 schema is defined.

**Not patched because checked and correct:** I6-S4 (P9.17 step 4 already
requires tracking error + rejected-command reasons in the shadow review);
I6-D1–D6 (owners exist or items are correctly labelled unverified).

## §7 — Verification (counts)

Before either iteration (verified by grep on 7 November 2026):

```
$ grep -c "^- \[ \]" ledger/phase-07-plugin-sdk/ledger_phase_7.md \
    ledger/phase-08-advanced-coordination/ledger_phase_8.md \
    ledger/phase-09-pilot-qualification/ledger_phase_9.md
ledger/phase-07-plugin-sdk/ledger_phase_7.md:32
ledger/phase-08-advanced-coordination/ledger_phase_8.md:30
ledger/phase-09-pilot-qualification/ledger_phase_9.md:30
```

After Iteration 5 patching: P7 39, P8 36, P9 36.
After Iteration 6 patching (this iteration): P7 43, P8 39, P9 41.
Post-patch counts are verified by re-running the same grep; see the parent
report-back for the final verified numbers.
