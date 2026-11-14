# P8.19 — Benchmark cases: extend §19.3 with research cases (and record what was not tested)

Date: 2026-11-14. Subtask: P8.19 (`ledger_phase_8.md` §P8.19). Owner role:
research lead. Report trace: §19.3; §19.4. Inputs: P8.01
(`01_readiness_check.md` — the baseline-case status and the G-1 blocking
rule); the §19.3 section of
`docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (read
verbatim — no case name is invented); `scenarios/library/` lib-01…lib-08
with their per-case IEEE exclusion records and pre-registered UNRUN
expected outcomes (`scenarios/library/INDEX.md`); the P3.26/3.27 scenario
artefacts (`scenarios/synthetic-portfolio/`); `benchmarks/README.md` (the
P3.27 harness rules — no-targets rule, `not_measured` never rendered as 0);
the P8.09/P8.12/P8.15 evaluation needs per the ledger's inputs line.

## 1. Status of this record — WRITTEN-UNRUN

The deliverable is the **versioned benchmark set + not-tested list**:

- `benchmarks/research-set-v1.json` — the frozen artifact, version id
  **`flexcore-research-benchmark-set-v1`**, deterministic field order;
- `benchmarks/research-set-v1-not-tested.json` — the not-tested list,
  part of the same frozen version;
- `benchmarks/README.md` — an ADDITIVE section describing both (the
  P3.27 harness record above it is unchanged).

**Nothing in the set has been RUN.** The workspace has never been built
or executed (P8.01 §1: the workspace test suite and the §19.3 benchmark
cases are UNRUN; G-1). Every `run_status` in the artifact is `NOT_RUN`
and every pre-registered expected outcome is UNRUN. The artifact freezes
the CASE LIST — it records no measurement, and filling a run never edits
it: the evidence lives in the evaluation records that cite the version
id. The §19.4 numeric success thresholds are NOT recorded here (still
missing — X-8; see §7).

## 2. The §19.3 base cases as realized in this repository (ledger step 1)

The report §19.3 list, read verbatim, with each bullet traced to what
actually exists. **A §19.3 case that is not listed here is not run** —
and the two IEEE bullets are listed and recorded as NOT INCLUDED, which
is the honest form of "listed":

| §19.3 bullet | Realized as | Status |
|---|---|---|
| IEEE 13-bus, 34-bus, 123-bus distribution feeders (where licensing and model availability are confirmed) | NOT INCLUDED — the per-case IEEE exclusion record (carried in every `scenarios/library/lib-*.json`, `ieee_case_record.included=false`) records: no IEEE model available in the repository, licensing and model availability unconfirmed; all cases are SYNTHETIC fixtures | NOT RUN |
| IEEE 39-bus and 118-bus transmission test systems for selected grid studies | NOT INCLUDED — same per-case exclusion record. **The IEEE 39-bus and IEEE 118-bus transmission cases are named explicitly here and in the artifact (§19.3 names them), and the honest record is: NO transmission case has been RUN in this repository — no transmission model, no transmission study, no transmission-class result exists** (the §19.1 PyPSA track has no integration) | NOT RUN |
| Synthetic portfolios with BESS, PV, flexible load, and EVSE | `lib-01-minimal-single-asset` (v1: one BESS, 24 h, 15-min cadence, scripted backend, three-bus radial fixture) and `lib-02-small-portfolio` (v1: two sites, BESS+PV+flexible load+EVSE+meter per site, 24 h, 1-min cadence, `scenario-v2.json` + `forecast-fixture-v1.json`); the P3.26 artefacts (`scenario-a/b/c`, v1) are the pinned inputs | NOT RUN (pre-registered outcomes UNRUN) |
| Multiple feeders/sites connected through a common substation constraint | `lib-03-medium-multi-site-meshed` (v1: the balanced MESHED two-feeder fixture `synthetic-meshed-two-feeder` v1 — one substation, two LV feeders, two mesh ties; site-north at bus_3, site-south at bus_5; `TopologyScope::RadialAndMeshed`, P4.34) | NOT RUN |
| Stressed scenarios (PV ramps, forecast error, communication loss, asset dropout, reverse power flow, topology change) | The research additions, §3 below (lib-04…lib-08 + the P5.21 fault-injection suite; two of the six stressors map PARTIALLY, recorded honestly) | NOT RUN |

Common parameters across the realized cases (verbatim from the records):
`master_seed` 20260601 with the recorded derivation
`seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`;
solver pins HiGHS 1.15.1 (LP/QP/MIP — never SOCP), Clarabel.rs 0.11.1
(continuous conic — no integer variables), pandapower 3.5.6 (provider
pin; API usage UNVERIFIED until the human build/test pass); objective
`SYNTHETIC_ENERGY_COST_DEVIATION_V1` (the declared P3.14 set has exactly
one entry; the sim-runner composition does not solve — D1/D2 OPEN).
Units are canonical (§10.2): MW/MVAr/MVA/MWh; `P > 0` = export at the
point of connection; SOC a fraction in [0,1].

**The D3 obligation** (P8.02 §4): the artifact declares the network
class — balanced distribution, radial default RECOMMENDATION plus one
balanced meshed fixture (lib-03). Unbalanced and transmission classes
are out of the declared scope (not-tested NT-5, NT-2/NT-3). D3 remains
OPEN; this declaration is a label on scope, not a decision.

## 3. The research-case additions (ledger step 2)

The six §19.3-named stressors, mapped to what the repository actually
holds — including the two PARTIAL mappings, stated as partial rather
than silently claimed:

| Stressor | Case / record | Parameters (verbatim source) | Honest mapping note |
|---|---|---|---|
| PV ramps | `lib-06-stress-pv-ramp` (v1) over `scenarios-phase3/scenario-b-pv-ramp.json` (v1) | PV down 80 percent of clear-sky inside 12:00–13:00 UTC; the stressor is the FORECAST, not the telemetry (telemetry stays all-GOOD, byte-identical to scenario (a)'s) | direct |
| Forecast error | `lib-06` over `forecast-fixture-v1-error-pv-ramp.json` (version `v1-error-pv-ramp`) | the P3.26 error variant of the same ramp | direct (single deterministic error fixture — no ensemble; NT-8) |
| Communication loss | `lib-07` drop window + `lib-04` worker timeout + the P5.21 fault-injection suite (dropped/reordered/duplicated messages, §19.2(6)) | no frames reach the platform for site-north-bess-1 inside 12:00–13:00 UTC; lib-04: bounded-timeout fail-closed | **PARTIAL** — analogues at the telemetry/worker layers; no dedicated sustained-loss case exists; the P5.21 suite is WRITTEN-UNRUN; the worker_process backend is an UNVERIFIED pandapower API (NT-7) |
| Asset dropout | `lib-07-stress-asset-dropout` (v1) over `scenarios-phase3/scenario-c-dropout.json` (v1) | drop window 12:00–13:00 UTC on site-north-bess-1; EXPECTED gap (P2.25 pre-registered MISSING/expected_gap); prior-schedule roll pin so the REAL P3.24 rolling roll fires | direct |
| Reverse power flow | `lib-08-stress-reverse-power-flow` (v1) over `fixtures/scenario-minimal-single-asset.json` (v1) | dispatch target +1.5 MW EXPORT (`P > 0` export, §10.2/P0.07); target_q 0.0 MVAr | direct, with the record's own honest limit: on the scripted backend the grid check is the recorded FIXTURE outcome and passes — the scripted backend CANNOT produce a reverse-power-flow violation; the violation expectation is pre-registered for the worker leg (UNRUN) |
| Topology change | `lib-03` (the meshed fixture) | topology-CLASS variation (radial vs balanced meshed) at fixture scale | **PARTIAL** — the lib-08 record states it itself: "a fixture-level topology class, not a runtime switching model"; a runtime topology-change event is NOT implemented (NT-6) |

Two additional library stress cases are in the set because the §19.3
stressed-scenarios bullet and the library's own `benchmark_reference`
fields put them there: `lib-04-stress-worker-timeout` (solver-timeout
analogue) and `lib-05-stress-nonconvergence` (non-convergence injection;
AC power-flow convergence is the §19.4 grid-feasibility metric). Both
are `worker_process` legs — written, never executed.

## 4. The versioned artifact and the anti-silent-growth rule (ledger step 4)

`benchmarks/research-set-v1.json` freezes the set with:

- **version id `flexcore-research-benchmark-set-v1`** — every Phase 8
  evaluation (P8.06 baseline runs, P8.09, P8.12, P8.15, P8.21, P8.23)
  cites exactly this ONE version id on every reported row; a row
  without the version id is invalid and must not be entered; an
  evaluation whose cited version differs from the artifact it claims to
  use is a named finding, never absorbed;
- **deterministic field order** — the artifact's field order is
  normative (stated in its `schema_note`); reviewers diff versions
  textually;
- **the anti-silent-growth rule, recorded in the artifact itself**: a
  benchmark change — an added, removed, or re-parameterised case, a new
  fixture version, a changed solver pin — is a NEW version
  (`flexcore-research-benchmark-set-v2`), frozen as a NEW artifact file
  with its own not-tested list. This mirrors the P8.06 baseline
  discipline (a baseline change is a new baseline version, never a
  silent edit — `05_06_harness_baselines.md` §3) and the P8.05
  `HARNESS_VERSION` discipline. The not-tested list's named candidates
  (NT-4 IEEE feeders, NT-6 runtime topology change, NT-7 sustained
  communication loss, NT-8 forecast ensembles, and any transmission
  case) are the recorded v2 candidates — none is silently promoted.

The evaluation-records linkage, recorded honestly: P8.09
(`09_admm_evaluation.md` §2.1) and P8.12 (`12_gnn_evaluation.md` §4)
were written against "the P8.19 versioned research-case extensions"
before this artifact existed; their filling rules already require the
benchmark-set version per row, and the version id they must cite is now
resolved to `flexcore-research-benchmark-set-v1`. Their records are NOT
edited by this subtask (file ownership); the linkage is recorded here
and checked by the ledger's verification method (each evaluation's
cited version against the artifact).

## 5. The not-tested list (ledger step 3)

`benchmarks/research-set-v1-not-tested.json` — eleven items, each with
its reason and its prohibition reference. The ones the ledger and §19.3
name explicitly:

- **NT-1 — a 10,000-bus utility-scale network**: NOT RUN, and a scale
  claim from the tested range is PROHIBITED. §19.3's closing sentence
  ("Do not claim that the first distribution feeder tests establish
  capability for a 10,000-bus utility network") is the source; the
  tested range is fixture-scale synthetic feeders and the ×2/×4
  in-memory portfolio copies. Enforced as P8.02 X-7 and restated in the
  P8.21 scaling protocol.
- **NT-2 / NT-3 — the IEEE 39-bus and 118-bus transmission systems**:
  named explicitly per §19.3 bullet 2; no transmission case has been
  RUN here (the per-case IEEE exclusion record).
- **NT-4 — IEEE 13/34/123-bus distribution feeders**: the §19.3
  precondition (licensing and model availability confirmed) is
  unconfirmed.
- **NT-5 — unbalanced distribution networks** (D3 OPEN), **NT-6 —
  runtime topology-change events**, **NT-7 — sustained communication
  loss as a dedicated case**, **NT-8 — probabilistic forecast-error
  ensembles**, **NT-9 — the PyPSA transmission/planning track**, **NT-10
  — dynamic-stability cases** (the P8.16/P8.17 ANDES scope; a
  steady-state tool is never transient-stability evidence, X-5), **NT-11
  — hardware-in-the-loop** (§19.2(10) gate order).

## 6. Reviewer-verification procedure (the human gate)

```sh
# (a) the documented-tools gate (the only tools this loop ran — output quoted below):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) artifact integrity (deterministic parse + version id + the honest statuses):
python -I -c "import json;d=json.load(open('benchmarks/research-set-v1.json'));assert d['version_id']=='flexcore-research-benchmark-set-v1';assert all(c.get('run_status')=='NOT_RUN' for c in d['research_case_additions']);print('set v1 OK')"
python -I -c "import json;d=json.load(open('benchmarks/research-set-v1-not-tested.json'));assert len(d['items'])==11 and d['items'][0]['item_id']=='NT-1';print('not-tested OK')"

# (c) the trace check (the ledger's acceptance: no invented case names):
#     every case_id in research-set-v1.json must resolve to a file at the
#     recorded path with the recorded version — spot-check lib-01, lib-03,
#     lib-06, lib-07, lib-08 against scenarios/library/ and the phase3 artefacts.

# (d) the citation check (the ledger's verification method):
#     each Phase 8 evaluation record's cited benchmark version must equal
#     flexcore-research-benchmark-set-v1 — checked at P8.09/P8.12/P8.15/
#     P8.21/P8.23 first-run fill time against this artifact.

# (e) FIRST RUN — the fills (human/lab inputs; the runs, not this record,
#     produce evidence; a filled run never edits the v1 artifact):
cargo build -p flexcore-sim-runner
cargo test -p flexcore-sim-runner          # the P5.18/5.19/5.20 library suites
# base + stress cases via the pinned ScenarioRecord v3 path (replay identity):
cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-01-minimal-single-asset.json --double-run
cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-02-small-portfolio.json --double-run
cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-03-medium-multi-site-meshed.json --double-run
cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-06-stress-pv-ramp.json --double-run
cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-07-stress-asset-dropout.json --double-run
cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-08-stress-reverse-power-flow.json --double-run
# worker_process legs additionally need the Python worker env (pandapower 3.5.6,
# UNVERIFIED API) and the CLI worker flags; lib-04/lib-05 run only after that gate:
#   cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-04-stress-worker-timeout.json --double-run --worker-program python --worker-script apps/worker/python/grid_worker.py
#   cargo run -p flexcore-sim-runner -- replay-run --record scenarios/library/lib-05-stress-nonconvergence.json --double-run --worker-program python --worker-script apps/worker/python/grid_worker.py
# the P3.27 benchmark harness over the pinned P3.26 artefacts (raw numbers only):
cargo run -p flexcore-sim-runner -- benchmark --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-a-normal-day.json --code-version <version> --out bench-a.json
cargo run -p flexcore-sim-runner -- benchmark --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-b-pv-ramp.json --code-version <version> --out bench-b.json
cargo run -p flexcore-sim-runner -- benchmark --scenario scenarios/synthetic-portfolio/scenarios-phase3/scenario-c-dropout.json --scaling 2,4 --code-version <version> --out bench-c.json
```

A green (a)–(d) earns only a one-line "human verification gate: PASSED
(date)" appended here; failures are recorded verbatim and the ledger
tick reverts to `- [ ]`. The (e) runs fill the evaluation records that
cite this version — they never edit the frozen artifact.

## 7. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Benchmark set versioned | DONE — `benchmarks/research-set-v1.json`, version id `flexcore-research-benchmark-set-v1`, deterministic field order, anti-silent-growth rule recorded in the artifact |
| Every evaluation cites it | RULE RECORDED (§4; the artifact's `evaluation_citations`); the citation check is the ledger's verification method, run at each evaluation's first-run fill — no evaluation row exists yet to check |
| Not-tested list exists and names at least the scale cases not run | DONE — `benchmarks/research-set-v1-not-tested.json`, NT-1 (the 10,000-bus network) first, NT-2/NT-3 (the named transmission cases) with the honest "no transmission case has been RUN" statement |
| §19.4 thresholds | NOT RECORDED HERE — still missing by design (X-8; P8.02 §5: thresholds are pre-registered after the model, service, benchmark, and operational risk are defined, at P8.06 freeze time at the earliest). The benchmark freeze is one of the four inputs that precondition names |

## 8. Contract gaps → follow-ups (recorded, not invented)

1. **No IEEE model and no licensing confirmation** (NT-2/3/4): the
   per-case IEEE exclusion record is the honest entry; inclusion is a
   licensing/model-availability task outside this phase's authority —
   a v2 benchmark-set candidate, never a silent addition.
2. **No runtime topology-change model** (NT-6) and **no sustained
   communication-loss case** (NT-7): recorded as v2 candidates; the
   quasi-static grid model has no switching event, and the P5.21 suite
   (the full fault set) is WRITTEN-UNRUN.
3. **The worker_process backend is an UNVERIFIED pandapower API**
   (P8.01 §4 item 4): lib-04/lib-05 and the lib-08 worker leg cannot
   produce evidence until the human build/test pass executes them.
4. **The P8.09/P8.12 templates predate this artifact** (§4): their
   `RECORD-AT-FIRST-RUN` benchmark-set-version fields resolve to
   `flexcore-research-benchmark-set-v1` at fill time; their records are
   not edited by this subtask (ownership).
5. **The P8.15 MARL evaluation** (concurrent owner) is expected to cite
   this version id; the citation check (the ledger's verification
   method) confirms it when that record's rows fill. Not asserted as
   done — expectation recorded only.

## 9. Non-claims (restated)

No case in this set has been RUN; the artifact records a case LIST, not
a result. No transmission result exists (no IEEE 39/118-bus run — named
honestly). No scale capability follows from the tested range (§19.3;
X-7). No universal target and no §19.4 threshold is recorded (X-8).
Nothing here qualifies any feature for field use (§2.1); the entire set
is simulation-only (AGENTS.md rule 5). Solver-class discipline holds
verbatim (HiGHS LP/QP/MIP; Clarabel conic; AC power flow is not AC-OPF).
