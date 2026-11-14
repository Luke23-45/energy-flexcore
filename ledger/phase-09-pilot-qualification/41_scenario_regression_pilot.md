<!-- P9.41 — Scenario-regression suite re-run on the pilot configuration (execution template). RE-RUN TEMPLATE ONLY: every run-record cell EMPTY, status UNRUN. The entire §22.2 scenario-regression machinery (all five legs) is WRITTEN-UNRUN — no baseline run exists anywhere, so no re-run exists and none is claimed. No pilot-config scenario variant is created (no pilot data exists to fill one). A regression-free result vs a frozen baseline ≠ correctness ≠ field readiness (AGENTS.md §3 discipline). -->
# P9.41 — Scenario-Regression Suite Re-Run on the Pilot Configuration (Execution Template)

Date: 2026-11-14. Subtask: P9.41 (`ledger_phase_9.md` §P9.41).
Owner role: Integration engineering.
Report trace: §23 Stage 9; §22.2 (the scenario regression suite, one of the
listed CI quality gates); §19.2(5) (deterministic scenario tests: pinned
inputs, solver versions, random seeds, expected outcomes/tolerances);
§19.2(6) (fault injection); §24 step 10 (replay with the same versions).
Inputs per the ledger: **P9.03** grid model (TEMPLATE — pilot grid model
`<EMPTY>`, `03_network_model_build.md`); **P9.04** integration design
(TEMPLATE — adapter set `<EMPTY>`, `04_customer_integration_design.md`);
**P9.14** lab results (NOT EXECUTED); the suite definition (P7.43 and the
Phase 1–6 suite artifacts it builds on). **None of the pilot inputs
exists.**

## 1. Honest status — the plain statement

**No scenario-regression run has been executed in any configuration —
baseline or pilot — and none is claimed.** The ledger's premise ("re-run
… as the Phase 5/6 baseline runs") has a precondition this record states
first: **the Phase 1–8 scenario suites were never executed.** Every leg
of the §22.2 machinery was delivered WRITTEN-UNRUN (the Phase 1–8
delivery loops banned builds and runs; green CI runs are recorded human
gates — `plugins/scenarios/README.md` §Status; P7.43 step 4 "first green
run … UNRUN"). There is therefore no executed baseline to re-run
against, and a pilot-config re-run cannot exist before a recorded
baseline run exists.

Under ENTRY_NOTE boundary 6, this record delivers what an agent can
deliver: the **suite inventory** — exactly which scenario sets exist, by
citation, with their suite/artifact identities, seeds, tolerances, and
runners (§2); the **pilot-config deltas** the re-run needs, all EMPTY
until P9.01/P9.03/P9.04/P9.16 fill them (§3); the **re-run procedure**
(same cases, same seeds, same version freeze — a changed baseline
invalidates the comparison, §4); the **tolerance rule** (X-8
pre-registration; the suites' own pinned expectations are never edited,
§5); and the **run record as an EMPTY template** (§6) whose fill is the
human gate. Nothing was run, probed, or executed in this writing loop.

## 2. The suite inventory — what exists and applies, recorded honestly

The §22.2 "scenario regression suite" is not one artifact; it is the
platform's scenario-regression PATTERN instantiated per phase
(`plugins/scenarios/README.md`: "the SDK leg of the platform
scenario-regression pattern (P5.20 `scenarios/library/` + P6.27 edge
wiring). It shares the PATTERN, not the scenarios"). The legs, by
citation — every one WRITTEN-UNRUN:

| # | Leg (suite identity) | Artifacts / suite files | Seed | Pinned tolerance / expectation rule | Runner | CI job | Status |
|---|---|---|---|---|---|---|---|
| L-1 | Phase 2 exit-gate scenario (P2.27) + the P4.31 fault/shadow suites | `scenarios/synthetic-portfolio/scenario-v1.json` (scenario_id `synthetic-portfolio-exit-gate`, artifact v1, telemetry scenario version v2); `crates/flexcore-application/tests/fault_injection.rs`; `crates/flexcore-application/tests/shadow_evaluation.rs` | master seed 20260601 travels with the versioned artifact; NO seed override | replay REPORT verdict must read `pass`; fault cases carry pre-registered outcomes (reject / hold / fallback — never a silent pass); shadow report carries its limits statement (§21.4) | `cargo run -p flexcore-sim-runner -- run --scenario scenarios/synthetic-portfolio/scenario-v1.json --out reference.json` then `… -- replay --scenario … --reference reference.json --report report.json`; `cargo test -p flexcore-application --test fault_injection`; `cargo test -p flexcore-application --test shadow_evaluation` | `scenario-regression` | WRITTEN-UNRUN |
| L-2 | Phase 3 deterministic scenario suite (P3.26) | `scenarios/synthetic-portfolio/scenarios-phase3/scenario-a-normal-day.json`, `scenario-b-pv-ramp.json`, `scenario-c-dropout.json` | master_seed 20260601 + recorded per-asset derivation rule (`seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`) | pre-registered expectations (anti-fitting: a first-run mismatch is a FINDING, never a reason to edit expectations); solver pins tier1 HiGHS 1.15.1 (LP/QP/MILP — never SOCP, AGENTS.md rule 4) asserted by the CI jq step | `cargo test -p flexcore-sim-runner --test phase3_scenarios`; the benchmark harness runs the same three scenarios for the record artifact | `phase3-scenario-regression` | WRITTEN-UNRUN |
| L-3 | Phase 5 replay library (P5.19/P5.20) | `scenarios/library/lib-01…lib-08` — ScenarioRecord v3 pinned replayable records (`INDEX.md`) | each record's `seeds.master_seed` = 20260601 with the derivation rule pinned IN the record | per-record `tolerances`: byte-identical for scripted deterministic legs; relative 1e-9 (floor 1e-12) for floating-point worker legs; expected outcomes pre-registered and UNRUN; replay reads recorded inputs ONLY (any irreproducible input is an error, never a silent default) | `cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-0N.json [--double-run \| --baseline <baseline-run-record.json>] --code-version <sha>`; exit 0 pass / 1 mismatch / 2 usage; a seed override is refused loudly | no dedicated job — the P5.19 reproducibility check joins the regression pipeline per the P5.20 record | WRITTEN-UNRUN (lib-04/lib-05 additionally need the Python worker environment — pandapower 3.5.6, never installed in any authoring loop) |
| L-4 | Phase 6 edge suite register (P6.27) | the 21-file register in `crates/edge-agent/tests/` (P9.37 §2.2, verbatim): fault-injection 9-case set (§19.2(6): `truncated`, `out_of_order`, `duplicate_delivery`, `clock_skew_future/past`, `stale_data`, `unmapped_point`, …), stale/clock-quality cases (`clock_quality_p6_31.rs`), command-path suites (`command_path_p6_14.rs` etc.); suite id `flexcore-edge-adapter-conformance-suite-v1` where the conformance claim is made | injected `FixedClock` / scripted ports — no seeded generation | per-case named verdicts; a mutated suite cannot be green; quality passthrough never upgrades (§10.4) | `cargo test -p flexcore-edge-agent --test <file>` per register file | `edge-agent-suite` (edge fault coverage lives here — P6.27 record §"Scenario regression suite" row) | WRITTEN-UNRUN |
| L-5 | Phase 7 SDK scenario-regression suite (P7.43) | `plugins/scenarios/sdk-01-manifest-to-plan.json` (6 steps), `sdk-02-profile-switch-with-drain.json` (35), `sdk-03-config-drift-refusal.json` (9), `sdk-04-proposal-envelope-replay-refused.json` (19) — 69 steps total; suite id `flexcore-plugin-sdk-scenario-regression-v1` asserted per scenario and printed per run | `pins.seeds` = "none" per scenario (pure structural kernel functions; no seeded generation on these paths) | `pins.tolerance` = exact — every expected outcome is a named refusal, an exact count, or a byte-identical determinism pin; the interpreter embeds the files at compile time (`include_str!`), so a mutated suite cannot be green | `cargo test -p flexcore-kernel --test scenario_regression_p7_43` | `sdk-scenario-regression` (the CI step logs `suite-id` + `suite-files`; a failing scenario blocks the merge) | WRITTEN-UNRUN |

**The baseline fact, stated once and plainly:** all five legs share ONE
status — WRITTEN-UNRUN. No green CI run, no local test pass, no
`replay-run` execution of any leg has ever been recorded. The ledger's
step 3 ("confirm the suite version run is recorded, so a later re-run is
comparable") therefore has a missing precondition this template makes
explicit: **the first human-executed run of each leg, on the frozen
pilot versions, recorded with its versions and seeds, IS the baseline.**
There is no earlier baseline to appeal to.

### 2.1 The ledger's four applicable categories mapped to the legs

Ledger step 1 names the regression scenarios applicable to the pilot as
four categories. The mapping (a leg is applicable to a category where it
exercises that behaviour; the applicability mark per leg is recorded in
§6.5 at execution, never assumed here):

| Category (ledger step 1) | Covered by |
|---|---|
| Deterministic scenarios | L-1 (exit-gate scenario), L-2 (the three P3.26 scenarios), L-3 (lib-01…lib-08 replay records) |
| Command-lifecycle transitions | L-5 (SDK lifecycle steps: `lifecycle-register/gate/begin-drain/complete-drain/state`, the drain machine — sdk-01/sdk-02); L-4 (edge command-path suites `command_path_p6_14`, `command_verify_p6_15`, `command_forward_p6_16`); L-1 (`shadow_evaluation` — §14.1 gate order) |
| Fault-injection subset | L-1 (`fault_injection` — P4.31 pre-registered reject/hold/fallback outcomes), L-3 (lib-04 worker-timeout, lib-05 non-convergence, lib-07 asset dropout), L-4 (the §19.2(6) 9-case fault set) |
| Stale/missing telemetry cases | L-1 (`fault_injection` stale-telemetry cases), L-4 (`stale_data` conformance case, `clock_quality_p6_31` hold-on-drift), L-3 (lib-07 dropout as the communication-loss analogue) |

L-5's composition/swap/config-refusal legs and L-3's stress legs are
part of the suite's regression surface; whether each is IN the pilot run
or EXCLUDED is a recorded decision per §6.5 — the ledger's named risk
("scenarios skipped as 'not applicable' without a reason") is structural
here.

## 3. Pilot-config deltas — ALL EMPTY until P9.01/P9.03/P9.04/P9.16 fill them

The suite artifacts are repo-generic (synthetic scenarios, the shipped
`openems-mapping-profile.v1.json` shape, reference solver pins). Running
them "against the pilot configuration" means running them with the
parameters below substituted — every one EMPTY today:

| Delta | Required value for the pilot re-run | Producer | Status |
|---|---|---|---|
| Pilot grid model | the built, validated pilot grid-model version (the model the L-2/L-3 legs' grid-feasibility legs must run against, if the pilot class differs from the synthetic fixtures) | P9.03 | `<EMPTY — model not built>` |
| Pilot adapter set / endpoints | the adapter(s) named by the signed P9.01 freeze, designed by the FILLED P9.04 (versions, profiles, endpoints — INSTALLED versions, never datasheets) | P9.01/P9.04 | `<EMPTY>` |
| Pilot mapping-profile variant | the profile variant the pilot runs, if the FILLED P9.04 dictates one (sign corrections after A-register verification, pilot channels) — created ADDITIVELY as a NEW file, the shipped v1 never modified (P6.10 D-10.1; P9.37 §3 rule verbatim) | P9.04 | `<EMPTY — v1 is the only file that exists; pilot variant NOT created>` |
| Site parameters | site ids, asset inventory, timezone, PCC — copied from the signed P9.01 freeze / P9.02 reconciliation, never substituted generically | P9.01/P9.02 | `<EMPTY>` |
| Frozen version set | the P9.16 frozen set: platform build (git SHA), edge build, worker versions, toolchain, solver pins AS RUN | P9.16 | `<EMPTY — NOT RECORDED>` |
| Lab environment | the P9.14 lab-first execution environment, then the pilot configuration | P9.14 | `<EMPTY — NOT EXECUTED>` |

**Scope rule — no scenario file is created or edited by this record.**
The ledger's deliverable is "Pilot-configuration scenario-regression run
record"; it dictates no new scenario artifact. Creating a pilot-config
scenario variant before P9.01/P9.03/P9.04 supply real data would
fabricate pilot configuration (the P9.37 §3 finding, applied verbatim).
If the FILLED inputs dictate a pilot variant scenario (e.g. the pilot
grid model substituted into a scenario's inputs), it is created
ADDITIVELY under the P7.43 suite-version discipline
(`plugins/scenarios/README.md` §Suite version): a NEW file, the
scenario's `scenario-version` bumped, and — when the scenario set
changes — the suite id and interpreter constant bumped together. The
existing suite JSONs (`plugins/scenarios/sdk-*.json`,
`scenarios/**/*.json`) are NEVER edited in place by the pilot work; a
mutated-in-place scenario is not a re-run of the suite, it is a
different suite wearing the old name.

## 4. The re-run procedure — same cases, same seeds, same version freeze

Ordered; each step is a human-gate act. The procedure ENFORCES the
anti-silent-growth discipline: the compared quantity is the suite AS
FROZEN — any change to cases, seeds, or versions between baseline and
re-run is not a deviation to absorb, it is the end of comparability.

1. **Preconditions gate.** §3's deltas are FILLED (P9.01 signed; P9.03
   model built; P9.04 design filled; P9.16 version set recorded; P9.14
   lab exists). A run started with any delta EMPTY is void — the ledger
   inputs (P9.03, P9.04, P9.14) are not optional.
2. **Applicability selection.** Mark every leg/scenario of §2 IN or
   EXCLUDED for the pilot run, with a recorded reason per exclusion
   (§6.5). Valid exclusion reasons are of the form "the pilot
   configuration does not exercise this layer" or "the leg's backend is
   unavailable and its exclusion is accepted as a limitation in P9.27"
   — never silence.
3. **Baseline run (the missing precondition).** Execute each IN-scope
   leg ONCE on the frozen pilot versions in the P9.14 lab-first
   environment, recording §6.1 per leg: suite/artifact identity +
   version, code version (git SHA), toolchain, solver pins AS RUN,
   seed(s) AS PINNED (no override — L-1's artifact seed and L-2/L-3's
   master_seed 20260601 travel with the artifacts; a seed override
   aborts, never re-rolls), environment, executor/witness, log
   reference. This recorded run is the comparison baseline. If a
   baseline for a leg already exists from an earlier recorded run at
   the SAME versions and artifacts, it is cited instead of re-executed.
4. **Pilot-config re-run.** Execute the SAME cases of each IN-scope leg
   with the pilot-config deltas of §3 substituted (pilot grid model,
   pilot adapter/profile variant, pilot site parameters) on the SAME
   frozen versions. Same artifacts, same seeds, same expectation pins —
   the ONLY differences from the baseline are the recorded pilot-config
   deltas.
5. **Comparability check.** Before comparing, re-verify identity:
   artifact hashes/versions, seed pins, and the frozen version set must
   equal the baseline's. Any difference — a scenario file edited, a
   seed overridden, a version bumped — yields
   `INVALID_RERUN_IDENTITY` for that leg (the P9.38 §3.1 input-identity
   class, applied here): the comparison does not happen and the leg is
   re-baselined as a NEW recorded run, never compared across a changed
   baseline.
6. **Per-scenario outcome recording (ledger step 2).** Every IN-scope
   scenario gets a recorded result: expected outcome (the artifact's own
   pre-registered expectation) vs actual (from the run output only).
   A mismatch is a FINDING dispositioned exactly one of: fixed and
   re-run / accepted limitation filed in P9.27 / blocks the pilot
   (ledger step 2). Editing the expectation to match the result is
   forbidden (§5; the P3.26 anti-fitting rule).
7. **Suite-version recording (ledger step 3).** Record per leg the
   suite/artifact identity and version AS RUN (§6.4) — for L-5 the
   suite id `flexcore-plugin-sdk-scenario-regression-v1` plus each
   scenario's `scenario-version`; for L-1/L-2/L-3 the scenario artifact
   id + version + seed + code version (the P2.13 §4 replay-identity
   triple); for L-4 the register-file set + the conformance suite id
   where cited. A later re-run is comparable only against this record.

Control status: the whole procedure is simulation/lab execution
(`§19.2(5)` layers; §21.4 simulation/read-only) — it grants no field
write authority and exercises no live site (§7).

## 5. Comparison tolerances — X-8 pre-registration (the P8.38/P9.38 rule)

- The suite artifacts carry their OWN pre-registered expectations and
  tolerance pins (L-1 verdict `pass`; L-2 pre-registered expectations +
  solver pins; L-3 per-record `tolerances` — byte-identical / relative
  1e-9 floor 1e-12; L-4 named verdicts; L-5 exact named verdicts). The
  re-run uses these AS PINNED and changes none of them: the machinery
  defaults are the records' own pins, not values chosen for the pilot.
- Any NEW comparison introduced only by a pilot-config variant scenario
  (§3) pre-registers its expectation and tolerance IN the variant
  artifact BEFORE its first run, with a recorded basis — never a
  universal target (X-8: `ledger/phase-08-advanced-coordination/02_research_scope_freeze.md`).
- A tolerance or expectation edited after seeing a result VOIDS the
  run for that scenario (the P9.38 §3.2 rule mirrored): the change is a
  NEW artifact version (suite-version bump per §3's scope rule), the
  affected scenario is re-run under the new version, and the record
  shows both — never a silent in-place edit.

## 6. Run record — EMPTY template (the human gate input)

**EVERY cell EMPTY. No leg has run; no baseline exists; no result, no
disposition, no suite-version record exists. Filling a cell from
anything other than actual run output of the actual frozen versions is
fabrication (AGENTS.md rule 1).**

### 6.1 Per-leg run identity (one block per executed leg)

| Field | Value (EMPTY — fill at the run) |
|---|---|
| Leg (L-1…L-5) | |
| Suite / artifact identity + version AS RUN | |
| Runner command + CI-job equivalent | |
| Code version (git SHA) / toolchain | |
| Solver pins AS RUN (HiGHS 1.15.1 tier1 where exercised) / worker environment (pandapower pin where exercised) | |
| Seed(s) AS PINNED (no override — abort rule recorded) | |
| Pilot-config deltas in force (from §3, cited) | |
| Environment (P9.14 lab-first / pilot config) | |
| Baseline-run reference (this run's recorded baseline, or an earlier same-version run cited) | |
| Execution date / executor / second-engineer witness | |
| Run log reference (file/commit — the log IS the run record) | |

### 6.2 Per-scenario result table (one row per IN-scope scenario; ledger step 2)

| Scenario id | Artifact version | Suite/leg | Seed | Expected (pre-registered, cited) | Actual (from run output) | Result (passed / failed / not-applicable-with-reason) | Detail |
|---|---|---|---|---|---|---|---|
| (one row per scenario — ALL `(EMPTY — UNRUN)`; the expected column is quoted from the artifact, never re-written) | | | | | | | |

### 6.3 Failures-and-dispositions table (ledger step 2)

| Failed scenario | Disposition (fixed-and-re-run / accepted limitation filed in P9.27 / blocks the pilot) | Evidence (log + diff reference) | Status |
|---|---|---|---|
| (none — the table exists only when a failure is recorded from actual output) | | | (EMPTY — UNRUN) |

A failed scenario with no disposition blocks P9.28 (ledger acceptance
criterion b).

### 6.4 Suite-version record (ledger step 3 — the comparability anchor)

| Field | Value (EMPTY — fill at the run) |
|---|---|
| L-1 artifact identity (scenario id + version + seed + code version, the P2.13 §4 triple) | |
| L-2 suite identity (the three scenario ids + versions + master seed + solver pins) | |
| L-3 record set + per-record `record_version`/`schema_version` | |
| L-4 register-file set (+ suite id where a conformance claim is made) | |
| L-5 suite id + per-scenario `scenario-version` | |
| Frozen version set (P9.16) equality check vs the baseline run | |
| Recorded-by / date | |

### 6.5 Applicability/exclusion record (the ledger's named risk, made structural)

| Leg / scenario | IN / EXCLUDED | Reason (recorded, per §4 step 2) | Accepted-limitation P9.27 ref (if excluded) |
|---|---|---|---|
| (one row per §2 leg and per L-3/L-5 scenario — ALL EMPTY) | | | |

## 7. What a passing re-run would and would not establish

A passing pilot-config re-run (every IN-scope scenario reproducing its
pre-registered expectation on the frozen pilot versions) would establish
ONLY: that the named build, on the named frozen versions, against the
pilot-config deltas of §3, behaves as the suite artifacts pre-registered
— i.e. **regression-free relative to a recorded frozen baseline** (and,
where no baseline existed, that the first recorded run matched the
pre-registered expectations). That is the §22.2 gate's actual content at
the pilot.

It would NOT establish any of the following, and a filled record may
claim none of them:

- **Correctness of the platform.** Regression-free vs a frozen baseline
  is consistency evidence, not correctness: if the baseline behaviour
  itself were wrong, the re-run faithfully reproduces the wrong
  behaviour. The finite scenario set covers only what it covers — a
  finite test set is not a guarantee (the X-4 discipline).
- **Field readiness or any field write authority.** The re-run is
  simulation/lab execution (§21.4); the pilot's control authority is
  the P9.26 control-scope document and the P9.28 acceptance decision —
  operator-approved AT MOST (v1.0.0 ladder), bounded automation
  post-1.0 (ENTRY_NOTE boundary 3). No result here changes that.
- **Safety, certification, or protection-grade capability** — AGENTS.md
  §3 rules 2–3, verbatim and binding: *"FlexCore is not certified, not a
  safety case, and not permission for live grid control"*; FlexCore is
  supervisory orchestration only (no primary frequency response,
  synthetic inertia, relay protection, inverter inner-loop, or
  sub-cycle control).
- **Adapter conformance.** The protocol-shape claim boundary is P9.37's
  (§7): the only permitted conformance wording is the recorded suite
  sentence; a scenario pass is not a conformance claim for any adapter,
  inverter, or site (§11.3 of the report, as cited in P9.37).
- **Comparability across a changed baseline.** A "pass" recorded against
  different artifacts, seeds, or versions than the baseline's is not a
  pass — §4 step 5 renders it `INVALID_RERUN_IDENTITY` by construction.
- **Reproducibility of anything not run.** Excluded scenarios (§6.5)
  carry their exclusion as a recorded limitation, not as covered.

A failing re-run is recorded AS THE OUTCOME (§6.3) — it does not
invalidate the platform; it invalidates the regression-free claim for
the affected scenarios until each failure is dispositioned.

## 8. Reviewer-verification procedure

1. Confirm §1's plain statement is unchanged and no executed-run claim
   exists anywhere in the record.
2. Confirm every §3 delta cell is EMPTY, every §6 cell is EMPTY /
   `(EMPTY — UNRUN)`, and no suite artifact was created or modified by
   this record (§3 scope rule; `git status` shows only this file + the
   ledger tick).
3. Traceability check (for a filled future record): every §6.2 result
   traces to a log reference that resolves; every version cell names
   the version AS RUN (installed, never datasheet); every seed cell
   matches the artifact's pinned seed (no override); every failure has
   a disposition (ledger criterion b); every exclusion has a recorded
   reason (§6.5); the §6.4 suite-version record exists (criterion c).
4. Anti-fitting check: no expectation or tolerance was edited after its
   result existed; any artifact change between baseline and re-run is
   recorded as a new artifact version with the comparability break
   visible — never absorbed (§4 step 5, §5).
5. Ledger verification method: a second engineer re-runs a SAMPLE of
   scenarios from the record alone (same artifacts, seeds, versions) and
   confirms the outcomes match — executable only because §6.4 pinned
   the identity.
6. Confirm §7's AGENTS.md §3 quotes are verbatim and the report trace
   (§22.2; §19.2(5)–(6); §24 step 10) resolves to real headings.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; no run of any leg has ever
   been executed.

## 9. Filling commands (for the human executor)

```sh
# (a) L-1 — exit-gate scenario + P4.31 fault/shadow suites (lab-first):
cargo run -p flexcore-sim-runner -- run \
  --scenario scenarios/synthetic-portfolio/scenario-v1.json \
  --out <run-dir>/reference.json --code-version <git-sha>
cargo run -p flexcore-sim-runner -- replay \
  --scenario scenarios/synthetic-portfolio/scenario-v1.json \
  --reference <run-dir>/reference.json --report <run-dir>/report.json
#     assert report verdict == "pass" (the verdict, not just the exit code)
cargo test -p flexcore-application --test fault_injection -- --nocapture
cargo test -p flexcore-application --test shadow_evaluation -- --nocapture

# (b) L-2 — Phase 3 deterministic suite:
cargo test -p flexcore-sim-runner --test phase3_scenarios -- --nocapture

# (c) L-3 — Phase 5 replay library (per record; --double-run for the
#     reproducibility proof, --baseline <file> against the recorded
#     baseline run record):
cargo run -p flexcore-sim-runner -- replay-run \
  --record scenarios/library/lib-01-minimal-single-asset.json \
  --double-run --code-version <git-sha> --out <run-dir>/baseline-run-record.json
#     (lib-04/lib-05 additionally require the Python worker environment —
#     pandapower 3.5.6 per apps/worker/python/requirements.txt, installed
#     and verified by the human executor, never in an authoring loop)

# (d) L-4 — Phase 6 edge register (per P9.37 §2.2 file list):
cargo test -p flexcore-edge-agent --test conformance_suite_p6_19 -- --nocapture
cargo test -p flexcore-edge-agent --test clock_quality_p6_31 -- --nocapture
cargo test -p flexcore-edge-agent --test command_path_p6_14 -- --nocapture
#     … plus the §6.5-marked register files

# (e) L-5 — Phase 7 SDK suite (69 steps; suite id printed per run):
cargo test -p flexcore-kernel --test scenario_regression_p7_43 -- --nocapture

# (f) THE PILOT-CONFIG RE-RUN (§4 steps 4–6): same commands, same
#     artifacts, same seeds, on the P9.16 frozen versions with the §3
#     deltas in force; every §6 cell filled FROM the run output only.

# (g) The documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

No cargo command has been executed in this writing loop (banned); every
run above is a human gate input, and its actual output — never this
template — fills §6.

## 10. Phase 7 follow-ups

1. **Named suite-version identity for the platform legs:** L-5 carries
   a first-class suite id (`flexcore-plugin-sdk-scenario-regression-v1`);
   L-1–L-4 are identified only by artifact id + version + seed + code
   version (the P2.13 §4 triple) and CI job names. If the pilot
   re-run needs a single citable suite id per leg (§6.4), the suite-id
   constant pattern is extended to those legs as a reviewed, additive
   change owned by the Phase 7 seam owners — never by renaming
   artifacts in place.
2. **Pilot-config scenario variant seam (§3):** if the FILLED
   P9.03/P9.04 dictate pilot variant scenarios, the additive variant
   files, their suite-version bumps, and the interpreter's handling
   (where applicable) are reviewed changes under the P7.43 discipline;
   the existing scenario JSONs are never edited in place.
3. **Second-engineer sample re-run tooling (§8 check 5):** the ledger's
   verification method needs the run record alone to determine the
   exact artifacts, seeds, and versions to re-run; the recorded-baseline
   + stored-input discipline of the replay machinery (P5.18 fail-loud
   pin verification) is the pattern the run-record format follows —
   whatever runner materializes must support executing a leg from its
   §6.1/§6.4 identity without other context.

## 11. Open risks carried to P9.27 — PROPOSED dated amendment (LR-212–LR-219)

The P9.27 register (`27_limitations_residual_risk.md`,
`pilot-limitations-register-v0.1-draft`, 2026-11-14) traces LR-001…LR-211
and predates this record (its trace sections end at records 40 and 22).
Per the register's own dated-amendment procedure ("every change is a
DATED AMENDMENT, never an in-place edit that loses the prior version"),
the rows below are PROPOSED as amendment **v0.2-draft** (section 3.x:
"From `41_scenario_regression_pilot.md` §11 (LR-212–LR-219)"). **The
register file is NOT edited here** — the proposal is recorded in this
subtask's record, and the register's owner applies it through the
register's own procedure.

| Proposed LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-212 | 41 §11 r1 | No baseline run exists — every suite leg is WRITTEN-UNRUN; the "re-run" cannot start until first human runs are recorded |
| LR-213 | 41 §11 r2 | Re-run compared across a changed baseline (artifacts/seeds/versions drifted) — the comparison is silently meaningless |
| LR-214 | 41 §11 r3 | Scenarios skipped as "not applicable" without a recorded reason |
| LR-215 | 41 §11 r4 | A mismatch "resolved" by editing the pre-registered expectation instead of dispositioning the failure (anti-fitting violation) |
| LR-216 | 41 §11 r5 | Suite run against lab adapter versions/config instead of the pilot's (the ledger's named risk) — a lab-green run misread as pilot evidence |
| LR-217 | 41 §11 r6 | Worker-leg environment gap: L-3's lib-04/lib-05 need the Python worker environment (pandapower 3.5.6) that has never been installed — the pilot run's worker legs cannot start without it |
| LR-218 | 41 §11 r7 | "Regression-free vs a frozen baseline" over-read as correctness or field readiness (§7) |
| LR-219 | 41 §11 r8 | A pilot-config variant created by editing an existing scenario JSON in place — a different suite wearing the old suite id (§3 scope rule) |

| # | Risk (this record's own register) | Why it is open |
|---|---|---|
| r1 | **No baseline run exists**: all five §2 legs are WRITTEN-UNRUN — no green CI run, no local pass, no replay-run execution has ever been recorded; the ledger's "re-run" premise is unmet until the first human runs exist | §1, §2; the ledger's step-3 precondition, made explicit |
| r2 | **Changed-baseline comparison**: any scenario-file, seed, or version drift between baseline and re-run silently voids comparability — rendered `INVALID_RERUN_IDENTITY` by §4 step 5, but the drift itself must be prevented by the frozen-version discipline (P9.16) | §4 steps 5–7; §6.4 |
| r3 | **Unrecorded exclusions**: a scenario dropped from the pilot run without a §6.5 reason is the ledger's named failure mode | §2.1, §6.5 |
| r4 | **Expectation back-fitting**: the strongest failure pressure on the suite — the pre-registered expectations are the suite's value, and an in-place edit converts the gate into a tautology | §5; the P3.26 anti-fitting rule |
| r5 | **Lab-vs-pilot version substitution**: the suite passes in the lab composition and is quoted as pilot evidence — the ledger's named risk; §3's delta table and §6.1 version cells are the check | §3, §6.1; ledger P9.41 risk row |
| r6 | **Worker-environment gap**: lib-04/lib-05's `worker_process` legs need the Python worker environment never installed in any authoring loop | §2 L-3; `scenarios/library/INDEX.md` |
| r7 | **Over-claim**: a green re-run read as correctness, safety, or field readiness | §7; AGENTS.md §3 |
| r8 | **In-place scenario mutation**: a pilot variant created by editing existing JSONs instead of an additive versioned file | §3 scope rule; `plugins/scenarios/README.md` §Suite version |

Status: **WRITTEN-UNRUN** — a scenario-regression re-run cannot be
produced by an agent in this loop; no baseline run, no pilot
configuration, no frozen version set, and no filled delta exists; every
run-record cell is EMPTY and stays EMPTY until the human execution fills
it from actual run output. Nothing was run, and no regression-free or
pass claim of any kind is made.
