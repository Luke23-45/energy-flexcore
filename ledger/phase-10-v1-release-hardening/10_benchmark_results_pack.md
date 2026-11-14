<!-- P10.10 — Benchmark results pack and thresholds record. TEMPLATE: the pack
     STRUCTURE and the pass/fail semantics are written; EVERY threshold cell is
     EMPTY (X-8: pre-registered BEFORE the first run — the registered-then-run
     ordering stated as BINDING, §2.3); EVERY measured cell is EMPTY (ENTRY_NOTE
     boundary 4 — no benchmark was run in this loop, nothing was built, nothing
     was measured). No fabricated number appears anywhere in this record. -->
# P10.10 — Benchmark Results Pack and Thresholds Record (TEMPLATE — every threshold EMPTY, every measured cell EMPTY)

Date: 2026-11-14. Subtask: P10.10 (`ledger_phase_10.md` §P10.10). Owner role:
Platform engineering (with power systems). Report trace: §19.4; §22.3 release
gate 8. Inputs: **P10.09 run records** (concurrent wave-2 sibling —
`ledger/phase-10-v1-release-hardening/09_benchmark_rerun.md` is its expected
path; at the time of writing it does not exist yet, and this record does NOT
cite numbers from it — there are none); the frozen benchmark set
`benchmarks/research-set-v1.json` (version id
**`flexcore-research-benchmark-set-v1`**, FROZEN, WRITTEN-UNRUN) and its
not-tested list `benchmarks/research-set-v1-not-tested.json`; the Phase 8
reproducibility standard
`ledger/phase-08-advanced-coordination/20_reproducibility.md` (the
environment-capture and record-schema requirements this pack inherits);
`benchmarks/README.md` (the no-targets rule, binding); ENTRY_NOTE.md boundary 4.

## 1. Status of this record — WRITTEN-UNRUN

This record is the **results-pack structure** (§3), the **thresholds record**
(§4 — every threshold EMPTY until pre-registered), the **pass/fail semantics**
(§5, defined BEFORE any result exists), and the **citation rule** (§6).
**No benchmark run exists against any release candidate; no §19.4 metric has
been computed; no threshold has been pre-registered anywhere in this
repository** (X-8 — the P8.38 pre-registrations are also still pending). The
release candidate itself does not exist as a built artifact (ENTRY_NOTE
boundary 1); P10.09's two full runs + the §24 vertical-slice run are UNRUN
human executions. Every measured cell in this pack is EMPTY and is filled
only by the human/lab gate via §9's commands. Nothing in this pack is a
safety proof (§19.4 forbids exactly that reading).

## 2. The registered-then-run ordering — BINDING (ledger acceptance criterion (b))

The ledger's acceptance criterion (b) is: "every threshold was recorded
before the P10.09 runs (evidence: dated threshold record)". This record makes
the ordering explicit and **binding**, not advisory:

1. **Thresholds first.** §4's threshold table is pre-registered — every
   applicable metric gets its threshold value AND its basis, in writing,
   dated, BEFORE the first P10.09 run starts. A threshold written after a
   result exists is void by construction: it is fitted, not pre-registered
   (the ledger's named risk "thresholds fitted to the results").
2. **Runs second.** P10.09's runs (two full runs + the §24 vertical-slice
   run) execute against the frozen release-candidate identifier only after
   the §4 record is complete.
3. **The date check is the enforcement seam.** The ledger's verification
   method ("check the threshold record's date against the P10.09 run dates")
   is executable because §4 carries a dated pre-registration row and §3's
   result rows carry dated run rows. A result row whose date precedes its
   metric's pre-registration date is recorded AS THE FINDING
   (`THRESHOLD_NOT_PRE_REGISTERED` for that metric) and the metric is
   unjudgeable — the run does not pass, it is out of order.
4. **No re-definition after results.** Once a threshold is registered and a
   run exists against it, the threshold is not redefined, widened, or
   re-based to make the result pass (the §19.4 no-targets rule and the
   P9.24 §4 judgement-rule analogue). A threshold that turns out to be wrong
   is a FINDING with a disposition, and re-judging happens only with a NEW
   dated registration plus a RE-RUN — never by re-reading the old numbers.

**Current status of the ordering:** step 1 has NOT happened (§4 is EMPTY),
so step 2 is not legitimately startable — any run performed before §4 is
filled is out of order by rule 3. This is recorded, not papered over.

## 3. The results-pack structure (per-benchmark result rows, EMPTY)

The pack is one row per benchmark case per run. Every cell is EMPTY; no row
exists. The structure below is normative for the pack as published with the
release-candidate identifier (ledger step 5).

### 3.1 Identity block (every row carries ALL of these — the citation rule, §6)

| Field | Content | Source |
|---|---|---|
| `benchmark_set_version` | exactly `flexcore-research-benchmark-set-v1` (the only frozen version; a different id is a NEW frozen artifact file, never an edit — `benchmarks/README.md` anti-silent-growth rule) | `research-set-v1.json` `version_id` |
| `code_version` | the code version the run executed (the §19.2(5) pin) | P8.20 §2 (`code_version` field) |
| `case_id` | the case key from the frozen set (e.g. a `lib-01`/`lib-02`/`lib-03` base case or a research addition row) — verbatim from the artifact, never paraphrased | `research-set-v1.json` |
| `run_id` / `run_ordinal` | which P10.09 run (first full run / second full run / §24 vertical-slice run) and the case instance | P10.09 record |
| `release_candidate_id` | the P10.18 candidate identifier (commit/tag/build ID) — the row is only valid against the frozen candidate | P10.01/P10.18 |
| `solver_pins` | the per-module required `component_versions` (HiGHS `1.15.1`, Clarabel `0.11.1`, etc., per the P8.20 §2 table) — absence of a required key is a finding, not an omission | P8.20 §2 |

### 3.2 Environment capture — per the P8.20 standard (verbatim discipline)

Every result row's environment block is captured at RUN time per
`20_reproducibility.md` §2 `environment` field, which itself carries forward
the `benchmarks/README.md` hardware/container rule:

| Field | Content | Refusal rule |
|---|---|---|
| `os`, `arch` | `std::env::consts` values | never guessed |
| `cpu_model` | `/proc/cpuinfo` on Linux; `unavailable: <reason>` elsewhere (recorded manually where the process cannot observe) | `unavailable:` is honest; omission and guessing are findings |
| `logical_cores` | `std::thread::available_parallelism` | never guessed |
| `memory` | `/proc/meminfo` on Linux; `unavailable: <reason>` elsewhere | same |
| `container_flag` | the `FLEXCORE_BENCHMARK_CONTAINER` override, the standard indicator files, or `no-indicator-found` — which is NOT proof of a non-container run | recorded verbatim, never resolved by assumption |

The full record additionally carries the P8.20 schema fields
(`flexcore-research/reproducibility-record.v1`): seeds with the verbatim
derivation string, `config.config_digest` (`sha256:<64 hex>` or
`fnv1a64:<16 hex>` — the algorithm travels with the digest),
`inputs.input_snapshot_ids`, `inputs.input_artifact_digests` (the recorded
data pipeline — "same seed, different data" is the named risk the digests
close), `artifacts.artifact_refs` (`flexcore-artifact://<key>` — a local
path is `ARTIFACT_REF_INVALID`), and `nondeterminism_sources` (REQUIRED
NON-EMPTY for every `within_tolerance`/`distribution` record). A result
whose reproducibility record is schema-invalid does not enter this pack.

### 3.3 Result rows — ALL CELLS EMPTY (no run exists)

| Case | Run | Candidate ID | Benchmark-set version | Code version | Solver pins | Environment block | Reproducibility-record valid? | Measured §19.4 values | Threshold(s) applied | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

Filling rules: cells come from the run's own outputs, clocks, and logs —
never from memory, never estimated; a case dropped from the frozen set
without a recorded reason is a finding (P10.09 criterion (d)); the two full
runs' agreement is judged within the PRE-REGISTERED tolerances (X-8), and a
deviation is investigated, never averaged away (P10.09 criterion (b)).

## 4. The thresholds record — EVERY VALUE EMPTY (X-8; ledger step 2)

Report §19.4, verbatim in substance and binding: "Do not set arbitrary
universal targets such as 'zero violations in 1,000 cases proves safety' or
'ADMM converges in 30 iterations.' Set acceptance thresholds after defining
the model, service, benchmark, and operational risk." Accordingly, NO
threshold value is recorded here. Each row below is pre-registered by the
human gate with a value AND its basis, dated; a threshold with no cited
basis is not deployable and is refused (`THRESHOLD_BASIS_REQUIRED`), exactly
as the P8.02 §138 discipline requires pre-registration after the model,
service, benchmark, and operational risk are defined.

**Pre-registration control row (all EMPTY):**

| Item | Value |
|---|---|
| Pre-registration date (must precede every P10.09 run date) | `<EMPTY — NOT PRE-REGISTERED>` |
| Registrant + independent reviewer | `<EMPTY>` |
| Model/service/benchmark/operational-risk definition the thresholds are set against (the §19.4 precondition) | `<EMPTY — the precondition documents do not exist yet>` |
| Metric-direction declaration per metric (which direction is "better") | `<EMPTY>` |

**The §19.4 metric areas applicable to the release (ledger step 1) —
threshold and basis cells EMPTY:**

| §19.4 metric area | Metrics | Threshold | Basis | Direction |
|---|---|---|---|---|
| Scheduling | objective value; schedule deviation; feasible interval count; runtime | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Asset model | SOC/power violations; ramp violations; availability prediction error | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Grid feasibility | voltage violations; branch/transformer overload; AC power-flow convergence; residuals | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Flexibility | available vs delivered MW/MWh; uncertainty calibration; shortfall | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Dispatch | acknowledgement latency; tracking error; failure/expiry rate; recovery time | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |
| Platform | availability; throughput; p95/p99 API/ingest latency; recovery; tenant isolation | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

Constraints on the pre-registration (recorded NOW, before any value exists):

1. **Units are canonical** (report §10.2): MW/MVAr/MVA/MWh; `P > 0` = export
   at the point of connection; SOC a fraction in [0,1]; voltage kV at the
   nominal base. A threshold expressed in any other unit is refused.
2. **No universal iteration/count targets** (X-8): "zero violations in N
   cases proves safety" and "converges in K iterations" are forbidden
   threshold SHAPES, not just forbidden values.
3. **Runtime thresholds are deployment-context thresholds, not universal
   ones** (`benchmarks/README.md` no-targets rule 1): a laptop/CI figure is
   never a deployment figure; the environment block (§3.2) is what keeps
   them separate.
4. **The scaling probe stays raw** (`benchmarks/README.md` rule 3): no
   scaling law, ratio, exponent, or trend is derived from the ×2/×4 points —
   the pack reports raw numbers only.
5. **The first-run threshold set is the only threshold set** for these runs;
   per §2 rule 4, changing it forces a new dated registration + re-run.

## 5. Pass/fail semantics — defined BEFORE any result exists (ledger step 3)

These semantics are fixed now, while every cell is EMPTY, so no result can
be judged under rules invented after seeing the result:

| Verdict | Definition (fixed pre-results) | Consequence |
|---|---|---|
| `PASS` | the measured value meets its pre-registered threshold in the metric's declared direction (≤ or ≥ as registered in §4's Direction column), on the metric's declared units | the row stands; consumed by P10.26/P10.27 |
| `MISS` | the measured value does not meet its threshold | REQUIRES a disposition — exactly one of: **fix** (and re-run), **waive with rationale** (signed, carried to P10.26/P10.27), or **block release** (goes to P10.27 as a blocker). A miss with no disposition is a pack defect |
| `NOT_MEASURED` | the metric produced no value for the case (solver leg absent, worker timeout, record structural failure) | recorded as the failure reason — NEVER as a zero, never as a blank without a reason (the `benchmarks/README.md` fail-loud completeness rule; no default zero exists) |
| `OUT_OF_ORDER` | the run date precedes the metric's threshold pre-registration date (§2 rule 3) | the metric is unjudgeable; the finding is the outcome |
| `BLOCKED_*` | a dependency refused the judgement (e.g. the reproducibility record is schema-invalid, the environment block is incomplete) | the refusal is recorded AS the refusal, never as a verdict |

Additional fixed semantics: an aggregate "pass rate" is NEVER published as a
substitute for per-metric results; a metric that did not meet its threshold
is not averaged, smoothed, or re-based; and the pack never converts a
per-case result into a claim about untested cases (§19.4 forbids the
"zero violations in 1,000 cases proves safety" reading as a conclusion).

## 6. The citation rule (ledger step 5 / P8.19 `evaluation_citations` rule)

**Every result in this pack names the benchmark-set version and the code
version** — exactly one benchmark-set version id
(`flexcore-research-benchmark-set-v1` for the v1 freezes) and one
`code_version` per row, plus the release-candidate identifier and the
per-module solver pins (§3.1). A result that cannot name all four is not
enterable in the pack (`CITATION_INCOMPLETE`): an evaluation whose cited
version differs from the artifact it claims to use is a named finding,
never absorbed (the anti-silent-growth rule). Any future benchmark-set
change is a NEW version id (`flexcore-research-benchmark-set-v2`, …) frozen
as a new artifact file — `research-set-v1.json` is never edited in place,
and this pack never silently cites a version that does not exist.

## 7. What the benchmark does NOT establish (ledger step 4 — the scope statement)

Recorded now, and restated verbatim-in-substance in the published pack:

- **The first distribution feeder tests do not establish capability for a
  10,000-bus utility network** (§19.3 closing sentence). The not-tested list
  (`benchmarks/research-set-v1-not-tested.json`) carries the 10,000-bus case
  first, with its reason, plus every other important case NOT run; the pack
  cites it and does not restate it as a capability.
- **No transmission case has been RUN** (the IEEE 39/118-bus systems are
  recorded honestly as NOT INCLUDED in the frozen set under the per-case
  IEEE exclusion records — `benchmarks/README.md` P8.19 section).
- **Network class:** the declared class of the frozen set is balanced
  distribution feeders (radial default, plus the lib-03 meshed fixture) —
  unbalanced topologies and transmission-class studies are out of the
  declared scope (the D3 obligation; D3 is OPEN).
- **Nothing here is a safety proof or a certification claim** (§19.4;
  AGENTS.md rules 2–3): benchmark results are simulation-only evidence at
  the tested scale, versions, and day. FlexCore remains a supervisory
  orchestration layer; the entire 0.x line and this v1.0.0 gate evidence are
  simulation/shadow-class (AGENTS.md rule 5; ENTRY_NOTE boundary 1).
- **A developer-laptop or CI-runner figure is not a deployment figure**
  (`benchmarks/README.md` rule 4); the environment block exists precisely so
  one cannot be presented as the other.

## 8. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: no threshold is registered anywhere, no
   result row exists, and every §3.3/§4 cell is EMPTY.
2. Confirm §2's registered-then-run ordering is stated as BINDING and §5's
   verdict vocabulary has no pass-path that skips a pre-registered
   threshold.
3. Confirm §4 contains NO numeric value in any Threshold/Basis/Direction
   cell (grep the table: no digits in those cells), and that the forbidden
   threshold SHAPES of X-8 are named.
4. Trace the citations: `benchmarks/research-set-v1.json` (`version_id` =
   `flexcore-research-benchmark-set-v1`), `research-set-v1-not-tested.json`,
   `benchmarks/README.md` (the no-targets rules), and
   `ledger/phase-08-advanced-coordination/20_reproducibility.md` §2 (the
   environment block and the record schema this pack inherits).
5. Confirm the §7 scope statement matches the ledger's step 4 and the
   not-tested list's first row.

## 9. Filling commands (for the human gate — exact order)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) FIRST (X-8 ordering, §2): pre-register §4 — every applicable metric's
#     threshold, basis, and direction, after the model/service/benchmark/
#     operational-risk definitions exist; date the registration; sign the
#     control row. A threshold without a basis is refused.
# (c) THEN run P10.09 (two full runs + the §24 vertical-slice run) against
#     the frozen release-candidate identifier; produce the per-run
#     reproducibility records (P8.20 §8 steps e/f) — validated records only.
# (d) Fill §3.3 row-by-row FROM THE RUN OUTPUTS ONLY (records, clocks,
#     logs); apply §5's verdict vocabulary; every MISS gets a disposition
#     (fix / waive with rationale / block release) in the same pass.
# (e) Check the ordering: every run date >= the §4 pre-registration date
#     (a violation is recorded as OUT_OF_ORDER, not re-dated).
# (f) Attach the §7 scope statement to the published pack with the
#     release-candidate identifier (ledger step 5); hand the pack to
#     P10.26 (release notes) and P10.27 (go/no-go).
```

## 10. Findings (recorded, not fixed)

- **F-P10.10-1 (structural, honest):** P10.09's run records do not exist yet
  (concurrent wave-2 sibling; expected path
  `ledger/phase-10-v1-release-hardening/09_benchmark_rerun.md`). This pack's
  §3.3 cites that path as the input it will consume; it contains no number
  from it because none exists.
- **F-P10.10-2 (X-8 dependency):** the §19.4 precondition documents (model,
  service, benchmark, and operational-risk definitions that thresholds are
  set AFTER) do not exist in the repository; the P8.38 per-module tolerance
  pre-registrations are also pending. Until both exist, §4 cannot be
  legitimately filled and P10.09 runs are out of order by §2 rule 3.
- **F-P10.10-3 (pack-level, not benchmark-level):** the pack inherits the
  P8.20 contract gaps that still stand — no object storage exists behind
  `flexcore-artifact://` references (a result whose artifacts are not
  retrievable through store references cannot be rerun-tested and therefore
  cannot enter the pack's claims), and environment capture is partial on
  non-Linux platforms (`unavailable: <reason>` is the honest entry, never a
  guess).
- **F-P10.10-4 (observed, no action taken):** `benchmarks/research-set-v1.json`
  declares every `run_status` `NOT_RUN` and the P3.27/P5 benchmark tables
  carry `not_measured` cells — consistent with this pack's EMPTY state; no
  discrepancy between the frozen artifact and this record exists.

## 11. Non-claims

No benchmark was run; no §19.4 metric was computed; no threshold value
exists in this record; no result is reproducible-verified (P10.09's rerun is
UNRUN); nothing here is a safety proof, a performance guarantee, or a
certification claim. The pack is a reporting structure with fixed judging
semantics — it establishes nothing until real runs fill it under the
registered-then-run ordering.

Status: **WRITTEN-UNRUN** — structure + semantics + EMPTY threshold record;
the thresholds, the runs, and the results are human-gate inputs that have
not happened and are not claimed.
