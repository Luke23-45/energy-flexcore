<!-- P10.09 — Reproducible benchmark — full rerun (decision record + rerun TEMPLATE). ENTRY_NOTE boundary 4: benchmark/measurement cells are EMPTY — every measurement, tolerance value, and run row is a human/lab gate input. The rerun record is EMPTY because NO benchmark has ever been run — that is the honest carried finding, not an omission. -->
# P10.09 — Reproducible Benchmark — Full Rerun (record + rerun TEMPLATE)

Date: 2026-11-14. Subtask: P10.09 (`ledger_phase_10.md` §P10.09). Owner role:
Platform engineering (with power systems). Report trace: §22.3 release gate 8
("simulator benchmark results are reproducible"); §19.3–§19.4. Inputs: P10.01
scope freeze (`01_release_scope_freeze.md`); the §19.3 benchmark cases as
frozen in `benchmarks/research-set-v1.json` (P8.19); the §24 vertical-slice
replay procedure; the P8.20 reproducibility standard
(`ledger/phase-08-advanced-coordination/20_reproducibility.md` — the rerun
test definition this record templates, not rewrites); the P8.38 tolerance
pre-registration registry (PENDING — §3 below). Deliverable: the rerun
TEMPLATE over the versioned benchmark set + the EMPTY rerun record + the
filling procedure. **No benchmark artifact was added to `benchmarks/`** — the
P8.19 anti-silent-growth rule forbids growing the frozen directory outside a
ledger step, and P10.09's steps are run steps, not artifact edits.

## 1. Honest status — the plain statement

**No benchmark has ever been run in this repository, and this record does not
pretend otherwise.** The frozen benchmark set itself carries the finding:
`benchmarks/research-set-v1.json` has `"status": "WRITTEN-UNRUN"`,
`"frozen_date": "2026-11-14"`, and the verbatim note: *"NOTHING in this set
has been RUN. The workspace has never been built or executed … Every
run_status below is NOT_RUN and every pre-registered expected outcome is
UNRUN."* There is therefore:

- **no first run** to compare a rerun against;
- **no reproducibility record** anywhere (P8.20 §1: "No rerun has happened
  and no reproducibility record exists anywhere in the repository");
- **no pre-registered tolerance value** (P8.38: tolerance VALUES EMPTY —
  registered before each module's first rerun, never fitted after);
- **no release-candidate build** the runs would execute against (ENTRY_NOTE
  boundary 1; the candidate identifier is `<PENDING P10.18>`).

The §22.3 gate-8 criterion "results are reproducible" is consequently
**NOT MET and NOT CLAIMED** by this record. What this record delivers is the
complete rerun TEMPLATE: the procedure, the identity rules, the tolerance
pre-registration discipline (X-8), the record shapes, and the exact filling
commands — so that the first human/lab run produces the evidence directly
into the §4 EMPTY tables without any post-hoc interpretation. No `cargo` or
benchmark command was executed in this loop (loop bans held; the deliverable
is written, never run).

| Field | Value |
|---|---|
| Release-candidate identifier | `<PENDING P10.18 — no candidate build exists>` |
| Benchmark-set version | `flexcore-research-benchmark-set-v1` (frozen 2026-11-14; NEVER edited in place) |
| Run 1 (full set) | `<EMPTY — never run>` |
| Run 2 (full set, clean environment) | `<EMPTY — never run>` |
| §24 vertical slice (steps 1–10) | `<EMPTY — never run>` |
| Step-10 replay | `<EMPTY — never run>` |
| Pre-registered tolerances | `<EMPTY — P8.38 registry PENDING; none exists>` |
| Reproducibility verdict | `<EMPTY — no runs to compare>` |

## 2. What is frozen — the versioned benchmark set this template reruns

The set is the P8.19 artifact, cited by version id on every future row (the
`evaluation_citations` rule in the artifact: *"Every Phase 8 evaluation that
reports numbers on cases in this set cites version_id
flexcore-research-benchmark-set-v1 on every row. An evaluation row without
the version id is invalid and must not be entered."*). Composition (all
`run_status: NOT_RUN`):

| §19.3 bullet | Cases | Included | Status |
|---|---|---|---|
| IEEE 13/34/123-bus distribution feeders | `ieee-13-bus-distribution`, `ieee-34-bus-distribution`, `ieee-123-bus-distribution` | **NO** — `included: false` | NOT_RUN; exclusion recorded per case (licensing and model availability NOT confirmed; no IEEE model exists in the repository) |
| IEEE 39/118-bus transmission systems | `ieee-39-bus-transmission`, `ieee-118-bus-transmission` | **NO** — `included: false` | NOT_RUN; "no transmission case has been RUN in this repository — no transmission model, no transmission study, no transmission-class result exists" (verbatim, the X-7 discipline) |
| Synthetic portfolios (BESS, PV, flexible load, EVSE) | `lib-01-minimal-single-asset`, `lib-02-small-portfolio` | YES | NOT_RUN |
| Multi-feeder / common substation constraint | `lib-03-medium-multi-site-meshed` | YES | NOT_RUN (balanced meshed fixture under the declared `TopologyScope::RadialAndMeshed`; the D3 network-class declaration in the artifact) |
| Stressed scenarios | `lib-04-stress-worker-timeout`, `lib-05-stress-nonconvergence`, `lib-06-stress-pv-ramp`, `lib-07-stress-asset-dropout`, `lib-08-stress-reverse-power-flow` + the 8 research-case additions (`ra-pv-ramps`, `ra-forecast-error`, `ra-communication-loss`, `ra-asset-dropout`, `ra-reverse-power-flow`, `ra-topology-change`, `ra-worker-timeout`, `ra-nonconvergence`) | YES | NOT_RUN |

**Anti-silent-growth (the rule a rerun must not violate, artifact-verbatim):**
*"A benchmark change is a NEW version. This mirrors the P8.06
frozen-baseline discipline … Every Phase 8 evaluation … cites exactly ONE
version id — this one for the v1 freezes. An evaluation whose cited version
differs from the artifact it claims to use is a named finding, never
absorbed."* A case added, removed, re-parameterised, or re-pinned is
`flexcore-research-benchmark-set-v2` frozen as a NEW file with its own
not-tested list (`benchmarks/research-set-v1-not-tested.json`, list id
`flexcore-research-benchmark-set-v1-not-tested`; NT-1 prohibits any 10,000-bus
scale claim — §19.3's closing sentence). **A dropped stressed scenario is
never silent** (the ledger's named risk): the acceptance criterion "(d) no
case was dropped from the set without a recorded reason" is enforced by the
version-id citation itself.

## 3. The X-8 tolerance pre-registration (the discipline the rerun enforces)

Report §19.4: *"Do not set arbitrary universal targets … Set acceptance
thresholds after defining the model, service, benchmark, and operational
risk."* The carried discipline (P8.38, X-8; P10.10 criterion b requires the
threshold record DATED BEFORE the P10.09 runs):

1. **Tolerance classes are declared; values are not.** `exact` for the
   deterministic modules and solver baselines (`baseline/highs-tier1-lp`,
   `baseline/clarabel-tier2-conic`, `baseline/rule-based-priority-list`,
   `admm-dmpc`); `within_tolerance` for ML INFERENCE reruns
   (`gnn-warm-start`, `gnn-screening`, `marl-policy`); `distribution` for
   TRAINING reruns. A class was declared per module in P8.38; the VALUE for
   every `within_tolerance`/`distribution` module is registered into the
   P8.38 registry BEFORE that module's first rerun, with its date — never
   fitted to the results, never derived by the comparator ("The comparator
   NEVER derives, defaults, or suggests a tolerance value" — P8.20 §4).
2. **No tolerance exists today.** The P8.38 registry is PENDING/EMPTY; a
   rerun of a non-exact module that starts before its tolerance is
   pre-registered is `INVALID_RERUN_IDENTITY` / `TOLERANCE_REF_REQUIRED`
   (the named refusals of the P8.20 schema) — the run is invalid, not
   lenient.
3. **The recorded risk this closes:** "A benchmark that 'reproduces' because
   the tolerance was widened after the fact" — the dated pre-registration is
   the evidence that the tolerance did not move; a widened tolerance is a
   NEW dated registration that RE-OPENS the comparison, never a silent edit.
4. **Unit discipline carries:** MW, MVAr, MVA, MWh; `P > 0` export at the
   point of connection; SOC fraction in [0,1] at the API boundary; voltage
   kV at nominal base, per-unit in solver representation (the artifact's
   `units` field, verbatim from §10.2). A tolerance expressed in different
   units than its metric is an identity violation.

## 4. The rerun procedure (the TEMPLATE — the human/lab gate fills §5)

Each step cites the seam that owns it; nothing here re-implements the P8.20
standard — it binds P10.09's five ledger steps onto it:

1. **Freeze the set for v1.0.0 (step 1).** DONE by P8.19 —
   `flexcore-research-benchmark-set-v1` is frozen and version-cited. If any
   case changes before the runs, freeze v2 first; P10.09 never runs against
   an edited-in-place artifact.
2. **Run 1 (step 2):** execute every INCLUDED case against the release
   candidate with pinned inputs, solver versions, random seeds, and expected
   outcomes/tolerances (§19.2 item 5). Identity requirements per row (the
   P8.20 §2 record fields): `record_schema_version`
   `flexcore-research/reproducibility-record.v1`; `benchmark_set_version`
   `flexcore-research-benchmark-set-v1`; `code_version` (the candidate
   identifier); `component_versions` — HiGHS `1.15.1` (Tier-1 LP baseline),
   Clarabel `0.11.1` (Tier-2 conic baseline), the torch/PyG pair and the RL
   framework as their recorded DECISION-POINT values (`UNPINNED-REVERIFY` is
   honest for an open decision point; absence is not), ANDES for the dynamic
   module; `seeds.derivation` from the frozen artifact's derivation string;
   `determinism.nondeterminism_sources` NON-EMPTY for every
   `within_tolerance`/`distribution` record (P8.38's register).
3. **Run 2 (step 3):** re-run the full set from a CLEAN environment (fresh
   containers/venv, same pinned versions) and compare per the P8.20 §4
   comparator rules: `exact` rows must match exactly; non-exact rows compare
   against the PRE-REGISTERED tolerance only. Record BOTH runs in full (the
   §5 tables). A deviation is investigated, never averaged away (the ledger
   acceptance wording); a non-reproduced row carries its exclusion marking
   (`NOT_REPRODUCED_MARKED_FOR_EXCLUSION` / `INVALID_RERUN_IDENTITY` /
   `BLOCKED_*`) — it is a recorded outcome, not a deletion.
4. **The §24 vertical slice (step 4):** run steps 1–10 END-TO-END against
   the candidate — tenant/portfolio/site/grid model + four simulated assets
   → telemetry with quality flags → 24 h flexibility → LP/MILP/QP schedule
   → feeder power-flow validation → PROPOSED command transitions →
   simulator-only dispatch → acknowledgement/tracking/partial
   failure/timeout → console audit trail — then run the step-10 replay with
   the same versions and verify reproducibility. **This slice is not
   substitutable by the piecemeal gate re-verifications (P10.03–P10.08)** —
   the ledger states this explicitly and this record carries it.
5. **Record everything (step 5):** configs, seeds, model versions, solver
   versions, and outputs per §7.4 benchmarking and §9.2 provenance — into
   the §5 EMPTY tables and the reproducibility-record store (per the P8.20
   §4 store procedure; never into `benchmarks/`).

## 5. The rerun record (EMPTY — the honest carried finding)

### 5.1 Full-set runs

| Case id | Run 1 outcome | Run 2 outcome | Pre-registered tolerance (P8.38 ref + date) | Max deviation | Verdict |
|---|---|---|---|---|---|
| (none — the set has never been run) | — | — | — | — | — |

### 5.2 §24 vertical slice (steps 1–10) and step-10 replay

| Step | Run outcome | Replay outcome | Notes |
|---|---|---|---|
| (none — never run) | — | — | — |

### 5.3 Exclusions / dispositions

| Case | Exclusion marking | Reason | Recorded by |
|---|---|---|---|
| (none) | — | — | — |

## 6. What a rerun would establish — and what it would not

| A completed rerun WOULD establish | It would NOT establish |
|---|---|
| That the INCLUDED cases of `flexcore-research-benchmark-set-v1` reproduce within PRE-REGISTERED tolerances across two clean environments (§22.3 gate 8, criterion a–c) | Any capability claim beyond the tested range: the first feeder/portfolio tests do NOT establish capability for a 10,000-bus utility network (§19.3 closing sentence; NT-1) |
| That no case was silently dropped (every row cites the version id; drops carry recorded reasons — criterion d) | A safety proof, a certification claim, or "zero violations in N cases proves safety" (§19.4 forbids this reading) |
| That the §24 slice runs end-to-end on the candidate and its replay reproduces (criterion e) | That the EXCLUDED IEEE distribution/transmission cases are covered (they are `included: false` with recorded exclusion reasons — a rerun of the v1 set says nothing about them) |
| That the thresholds were set before the runs (the dated P8.38 registrations + P10.10's dated threshold record) | Global optimality, SOCP exactness, or any solver-class claim — the solver-class rules (HiGHS LP/QP/MIP only; Clarabel convex conic only; IPOPT local only) are unchanged by reproduction |
| That ML/RL results are bounded by the disclosed nondeterminism sources (P8.38's register: GPU kernel reduction order, atomics, autotuned library algos, parallel data-loading order, framework threading, version sensitivity) | That a training rerun matches an inference rerun or vice versa (the tolerance-class rule refuses exactly that comparison) |

## 7. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) Repository verification (AGENTS.md §6) — the record's structural checks:
python tools/verify_docs.py
python tools/validate_agent.py

# (b) The P8.20 reproducibility tooling self-tests (hand-computed vectors;
#     these pin the SCHEMA and comparator, they are NOT run outcomes):
python apps/worker/python/reproducibility/seed_derivation.py
python apps/worker/python/reproducibility/record_schema.py
python apps/worker/python/reproducibility/rerun_check.py

# (c) THE RERUNS themselves — after P10.18 records the candidate identifier
#     AND P8.38 registers the dated tolerance values for every non-exact
#     module. The set's frozen record paths (scenarios/library/lib-*.json)
#     name each case's scenario/grid-model fixture and version; every run
#     row carries record_schema_version flexcore-research/reproducibility-
#     record.v1 and benchmark_set_version flexcore-research-benchmark-set-v1.
#     Run 1 on the candidate; Run 2 from a clean environment; the §24 slice
#     steps 1–10; the step-10 replay. Fill §5 — never benchmarks/.
```

Independent verification (the ledger's verification method): re-run at least
one feeder case, one synthetic portfolio case, and one stressed scenario from
the FILED configs and compare against the filed results; confirm the
benchmark-set version id cited on the rows matches the artifact used.

## 8. Findings (recorded, not fixed)

- **F-P10.09-1 (the honest carried finding):** the §22.3 gate-8 criterion
  "simulator benchmark results are reproducible" is NOT met — no benchmark
  has ever been run, no run pair exists to compare, and no tolerance is
  pre-registered. P10.10 (results pack) has NO run records to compute §19.4
  metrics from; both subtasks' evidence cells remain EMPTY until the human
  build/run gate executes §7(c). A go decision (P10.27) that reads gate 8 as
  met on the strength of this record would be a false claim.
- **F-P10.09-2 (recorded scope fact):** the §19.3 base-case list is only
  partially realizable in this repository — the IEEE distribution and
  transmission cases are `included: false` with recorded exclusion reasons
  (licensing and model availability unconfirmed; no IEEE model exists). The
  frozen set is therefore fixture-scale synthetic feeders plus the stress
  set. Any release-notes text (P10.26) claiming IEEE coverage would be a
  finding against it.
- **F-P10.09-3 (recorded constraint):** the D3 network-class declaration in
  the artifact is a RECOMMENDATION pending the D3 decision (`d3_status`:
  OPEN) — the declared class (balanced radial default + one balanced meshed
  fixture) governs what the included cases may claim until D3 closes.
- **F-P10.09-4 (recorded constraint):** `code_version` for the runs is the
  P10.18 candidate identifier; because the candidate does not exist, no run
  row can carry a valid identity — the runs are blocked upstream, not by
  this subtask.

## 9. Ledger tick

`P10.09` in `ledger_phase_10.md` was ticked in the same wave as this record,
with the ENTRY_NOTE boundary 4 meaning: **the rerun TEMPLATE is written and
the rerun record is EMPTY by design** — "implemented, awaiting human
build/run". No reproduction, no tolerance value, and no run outcome is
claimed anywhere in this record.
