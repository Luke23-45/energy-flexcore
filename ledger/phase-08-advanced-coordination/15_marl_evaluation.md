# P8.15 — MARL evaluation: held-out regimes, outages, delays, uncertainty (TEMPLATE — every measured cell EMPTY)

Date: 2026-11-14. Subtask: P8.15 (`ledger_phase_8.md` §P8.15). Owner role:
research engineer (RL). Report trace: §13.3; §19.3–§19.4; §19.2(9).
Inputs: P8.13 (`13_marl_design.md` — the frozen M3 question/hypothesis
form, the safety-filter scope, the infeasible-filter outcome) and P8.14
(`14_marl_worker.md` — the shadow worker this template evaluates, its
closed outcome vocabulary, and its `filter_probe` deterministic filter
oracle); P8.05 (`05_06_harness_baselines.md` §2 — the independent
validation harness); P8.06 (`05_06_harness_baselines.md` §3 — the frozen
baseline registry `baseline/highs-tier1-lp` /
`baseline/clarabel-tier2-conic` / `baseline/rule-based-priority-list`);
P8.02 (`02_research_scope_freeze.md` — question M3, prohibitions X-2/X-4/
X-8); P8.20 (the reproducibility record this template references); P8.19
(the versioned benchmark extensions); ENTRY_NOTE.md boundary 4.

## 1. Status of this record — a TEMPLATE, nothing measured, nothing fabricated

**Every measured cell in this document is EMPTY.** Per the phase
boundaries (ENTRY_NOTE boundary 4: "Every measurement is an EMPTY cell")
the evaluation runs are **human/lab gate inputs**: no constraint-violation
frequency, return figure, robustness statistic, filter activation rate,
infeasibility rate, or baseline comparison from a real run appears
anywhere below, and none was computed by any code in this loop (nothing
was built or run).

What exists and is verifiable to the extent the loop allows:

- the **aggregation harness**
  (`apps/worker/python/marl_worker/evaluation_harness.py`,
  `flexcore-marl-eval/1`) — pure stdlib functions over INJECTED shadow-log
  entries and INJECTED consequence records: the safety-filter statistics
  (activation rate, infeasibility rate, the outcome of EVERY
  infeasibility — ledger step 3) and the per-case §19.4 metric rows
  (constraint-violation frequency, return, distribution-shift deltas,
  per-metric baseline comparison — ledger step 2), each pinned by
  HAND-COMPUTED self-tests with the arithmetic stated in the test. The
  harness was **written, never run** (WRITTEN-UNRUN; no python beyond the
  four permitted verification commands executed in this loop), with its
  pytest mirror (`tests/test_evaluation_harness.py`) written and never
  run;
- the **filling procedure** (§8) — the exact commands the human/lab pass
  runs and the cells they fill.

**The test-arithmetic / evaluation-result distinction (stated explicitly):
** the self-tests assert hand-derived values over constructed inputs
(e.g. 1 activation in 5 actions is exactly 1/5 = 0.2; 2 violations in 4
decisions is exactly 2/4 = 0.5; a return delta of 9.0 − 8.0 = 1.0 under a
pre-registered 0.5 tolerance is a win). That is **test-grade arithmetic
pinning the metric definitions** — it is NOT an evaluation measurement,
NOT a seed result, and NOT evidence about any case. No measured cell is
filled by it.

## 2. The evaluation suite, frozen before any run (ledger step 1)

### 2.1 The case families (declared NOW; case ids frozen at the first run)

Each family names its baseline comparison from the frozen P8.06 registry.
**No case id is invented here**: the concrete case ids, the benchmark-set
version cited by every row, and the per-case parameters are
`RECORD-AT-FIRST-RUN` fields, frozen when the first run happens, checked
COMPLETE against this suite definition (the ledger's verification method:
"check the case list is complete against the P8.13/P8.15 suite
definition" — no case silently dropped). Adding a case after seeing
results requires a dated amendment here with the reason.

| Family (closed vocabulary) | What varies | § trace / source | Baseline comparison (frozen P8.06 id) | Status |
|---|---|---|---|---|
| `held_out_topology` | Topologies held out of any training exposure (the P8.10 §4 held-out rule applied to the MARL environment's feeder scope) | §13.3; §19.3 | `baseline/highs-tier1-lp` AND `baseline/rule-based-priority-list` (§13.3: deterministic optimization AND rule-based) | EMPTY — run pending |
| `held_out_regime` | Load/weather regimes not seen in training (P8.19 regime extensions) | §13.3; §19.3 + P8.19 | same pair as above | EMPTY |
| `device_outage` | Declared device outage cases (asset dropout mid-episode) | §13.3; §19.3 (asset dropout) | same pair | EMPTY |
| `message_delay_loss` | Delayed and lost messages between the coordination layer and the environment (bounded staleness, dropouts) | §13.3; §19.2(6) | same pair | EMPTY |
| `state_estimate_uncertainty` | Uncertain state estimates feeding the policy observation (declared telemetry-quality degradations — MISSING/STALE/ESTIMATED values per §10.2, never silently imputed per P8.13 §2.4) | §13.3; §19.2(9) | same pair | EMPTY |

Selection discipline (the ledger's named risks — evaluation only on
training regimes, delay/loss never tested): the family list above is
declared BEFORE any run and is frozen at the first run. A family with no
runnable case at first-run time is recorded as such in the table (with
the reason), never dropped silently.

### 2.2 The baselines (§13.3), and the PI gap

The per-module P8.06 mapping for MARL is: deterministic optimization
(`baseline/highs-tier1-lp`, HiGHS — LP/QP/MIP, **not SOCP**) AND the
rule-based baseline (`baseline/rule-based-priority-list`,
`priority-list.v1`), compared on the SAME cases with the SAME accounting
(§13.3; `05_06_harness_baselines.md` §3). The ledger's §13.3 wording
"rule-based/PI baselines" also names a **PI baseline, which has NO frozen
id in the P8.06 registry** — the aggregation harness refuses any baseline
id outside the frozen registry (`BASELINE_UNKNOWN`), so a PI comparison
is refused, not approximated, until P8.06 defines, runs, and freezes one
(recorded contract gap, §9.1 — never invented here). Centralized-training
baselines "where applicable" (§13.3) are likewise a P8.06 registry
question, not a local invention.

## 3. Pinned seeds/versions — the PROCEDURE (the full record is P8.20's)

Nothing is pinned in this loop (no framework exists to pin; ENTRY_NOTE
boundary 5). What is pinned is the PROCEDURE — every evaluated run must
carry, before its numbers are reportable:

| Item | Procedure | Owner record |
|---|---|---|
| Full reproducibility record | benchmark-set version (P8.19 artifact), framework versions (the P8.13 §5 DECISION-POINT — resolved by install at the human gate), policy artifact reference, training-data reference, dataset/preprocessing versions, seeds, configs, input snapshot IDs | **P8.20** owns the record; this template references it and never duplicates it |
| Seeds | the caller-declared training seed travels on every result and log entry (`training_seed`, P7.42 seed honesty); inference seeds per the P8.38 nondeterminism procedure | P8.38 |
| Policy version / environment version | recorded on every envelope and log entry (the closed P8.04 payload) | P8.04 |
| Benchmark-set version | every case row cites the P8.19 versioned artifact | P8.19 |
| Tolerances | pre-registered at the P8.06 freeze BEFORE any research number exists (X-8) — the harness refuses a verdict without them (`THRESHOLD_NOT_PRE_REGISTERED`) | P8.06 |

## 4. Per-case result table — ALL MEASURED CELLS EMPTY

One row per case; every measured cell is filled ONLY by executing §8's
commands, from the harness's outputs (never from memory, never
estimated). Each filled row must carry the pinned-version refs of §3 — a
number without them is invalid and must not be entered.

| Case id (RECORD-AT-FIRST-RUN) | Family | Decisions (N) | Constraint-violation frequency (violated/N) | Violation kinds (counted, NOT merged) | Mean return (declared unit) | P8.05 validation outcome | vs `baseline/highs-tier1-lp` (per-metric deltas + verdicts) | vs `baseline/rule-based-priority-list` (per-metric deltas + verdicts) | Robustness deltas (shifted − reference) | Notes |
|---|---|---|---|---|---|---|---|---|---|---|
| EMPTY | held_out_topology | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | held_out_regime | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | device_outage | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | message_delay_loss | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |
| EMPTY | state_estimate_uncertainty | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

Filling rules (recorded now, before any number):

- Cells come from `case_statistics`, `distribution_shift_robustness`, and
  `compare_case_to_baselines` outputs; an in-band refusal
  (`EVAL_LOG_EMPTY`, `VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED`,
  `CONSEQUENCE_SHAPE_INVALID`, `RETURN_UNIT_INCONSISTENT`, `BASELINE_UNKNOWN`,
  `THRESHOLD_NOT_PRE_REGISTERED`, `CASE_FAMILY_UNKNOWN`) is recorded AS
  THE REFUSAL in the cell — never as a blank and never as a number.
- The distribution-shift columns are filled per family pair (reference =
  the in-distribution case; shifted = the held-out/outage/delay/
  uncertainty case) with `violation_frequency_delta` and
  `mean_return_delta` — deltas only; the harness computes no robustness
  verdict without the pre-registered thresholds.
- The comparison columns carry PER-METRIC verdicts (win/tie/loss under
  the pre-registered tolerances). The two metrics are NEVER merged into
  one headline number (P8.13 §2.6; §19.4). Wins, LOSSES, and ties are all
  reported (the ledger's discipline; P8.09's filling rules mirrored).
- The "baseline comparison" is only valid where the baseline was run on
  the SAME case with the SAME accounting — a baseline cell that was not
  run is `NOT RUN (reason)`, never another module's number.
- Units are declared, never assumed: the return unit travels on every
  record (`return_unit`); power in MW, energy in MWh, SOC a fraction in
  [0, 1] (§10.2).
- Every reported policy proposal passed the P8.05 independent validation
  harness; a case whose proposals did not is recorded with the validation
  outcome, and its return/frequency cells carry that qualification.

## 5. Safety-filter statistics (ledger step 3) — computable NOW from the closed outcome vocabulary

The P8.14 worker makes the filter outcome MANDATORY and closed on every
shadow-log entry (`safety_filter.py::FILTER_OUTCOMES`:
`passed_within_limits`, `filtered_clamped_to_limits`,
`refused_permission_ceiling`, `refused_nonfinite_action`,
`infeasible_no_safe_projection`) — the ledger's named failure mode ("a
shadow log that records actions but not filter outcomes, making safety
unevaluable") is unrepresentable, so the statistics below are computable
from the log alone:

| Statistic | Definition (stated arithmetic) | Where computed |
|---|---|---|
| Activation rate | `filter_activation_count / total_actions` (entries with ≥1 clamp over validated entries) | `evaluation_harness.filter_statistics` |
| Infeasibility rate | `infeasibility_count / total_actions` | same |
| Refusal rate | `refusal_count / total_actions` over `REFUSING_OUTCOMES` | same |
| Outcome of EVERY infeasibility | one row per infeasibility occurrence: sequence, named outcome, affected assets, recorded flag, fallback facts | same (`outcome_of_every_infeasibility`) |

**The aggregation code** (`evaluation_harness.py`) is pure stdlib over
injected log entries: it validates the log first (`shadow_log.
validate_shadow_log` — strictly-increasing 1-based sequences, a gap would
hide an action), refuses an empty slice (`EVAL_LOG_EMPTY` — an empty
denominator cannot become a rate of zero), and computes the rates as
exact divisions pinned by hand-computed self-tests (1/5 = 0.2, 3/5 = 0.6
over the constructed five-entry log; 1/1 = 1.0 for the single-
infeasibility log). **The per-case §19.4 metrics are computed over the
same entries only when their consequences are RECORDED** — a
`not_simulated` slice is refused
(`VIOLATION_FREQUENCY_NOT_COMPUTABLE_FROM_NOT_SIMULATED`): an EMPTY
consequence cannot be silently converted into a violation frequency of
zero (that conversion would be exactly the §13.3 failure this phase
forbids).

| Ledger criterion | Harness coverage |
|---|---|
| activation rate | `filter_statistics(...)["activation_rate"]` — EMPTY until a run exists |
| infeasibility rate | `filter_statistics(...)["infeasibility_rate"]` — EMPTY |
| outcome of every infeasibility | `filter_statistics(...)["outcome_of_every_infeasibility"]` — EMPTY (the rows will list EVERY occurrence) |

Recorded contract gap (honest handling, not a silent default): the P8.14
log schema's `infeasibility` object carries `outcome` / `entries` /
`recorded`, but the fallback facts (`fallback_attempted`, `fallback_viable`)
live only in the filter REPORT, not in the log entry. The harness records
`NOT_RECORDED_IN_LOG_SCHEMA` for those facts rather than fabricating a
value; the additive log-schema extension is the recorded follow-up (§9.2).

**Safety-filter statistics table — ALL CELLS EMPTY:**

| Case id (RECORD-AT-FIRST-RUN) | Total actions | Activation count / rate | Infeasibility count / rate | Refusal count / rate | Outcome of every infeasibility (per occurrence) |
|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

## 6. The evaluation note (ledger step 4) — the limitation is written NOW, in plain language

The following statements are FIXED before any run exists (the note's
content beyond them is EMPTY until the runs fill it):

1. **The finite-test-set limitation, in plain language (§13.3; X-4 —
   verbatim discipline):** *Zero observed constraint violations from this
   evaluation suite is a test outcome about the tested cases under the
   tested conditions and the declared models. It is not a universal
   safety guarantee, not a certification, and not evidence that the
   policy is safe on cases, regimes, outages, delays, or uncertainties
   outside the suite. No finite test set can establish a safety
   guarantee, and none is claimed.*
2. **Proposals only (ADR-010; X-2):** every evaluated action is a
   proposal from a simulation/shadow worker with no code path to the
   dispatch gateway (enforced in the P8.14 worker, verified by its
   boundary tests). Nothing in this evaluation issues, authorizes, or
   approaches a device command.
3. **The infeasible-filter outcome is a design failure if it passes
   through unfiltered (H-M3):** a filter infeasibility that produced an
   unfiltered pass-through would falsify the pre-registered hypothesis
   outright — it is a design failure, not a result. The implemented
   filter's fallback is attempted and provably non-viable on the empty
   interval (P8.13 §4), and every infeasibility is recorded.
4. **Distribution shift is measured, not asserted:** robustness claims
   are limited to the tested shift families (held-out topologies,
   regimes, outages, delays, estimate uncertainty) — no generalization
   beyond the tested range (X-7's discipline applied to MARL).
5. **Solver-class discipline (AGENTS.md rule 4):** the deterministic
   baseline compared here is HiGHS LP/QP/MIP — not SOCP, not a conic
   solver; the rule-based baseline's strength is exactly its written
   rule set. A "win" against a weak baseline is visible as such through
   the P8.06 assumptions record.
6. EMPTY note sections: **where the policy helps** — EMPTY; **where it
   does not** (losses reported alongside wins) — EMPTY; **what would
   change the answer** — EMPTY (measured from the case matrix, not
   asserted).

## 7. What the harness computes vs what the human run supplies

| Supplied by | Content |
|---|---|
| `evaluation_harness.py` (written, never run) | the metric definitions and their refusal discipline; the filter statistics (rates + the outcome of every infeasibility) over injected log entries; the consequence-record shape (`build_case_consequence` — validates, never computes); per-case frequency/mean-return arithmetic; distribution-shift deltas; per-metric comparison verdicts under injected pre-registered tolerances; the reporting-only statement on every output; the no-dispatch-shaped-callable namespace pin |
| The human/lab run | the runs themselves (the P8.13 §5 framework decision, the P8.13 §3 CBF derivation sign-off — training is BLOCKED until both exist); the consequences per decision (recorded via `build_case_consequence`, `consequence_status` → `recorded_by_harness`); the frozen P8.06 baseline results on the SAME cases (which do not exist until the first run, `05_06_harness_baselines.md` §3); the pre-registered tolerances (P8.06 freeze); the wall-clock facts and P8.20 reproducibility records; the fills of §4/§5/§6 |

## 8. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the pure self-tests (no framework needed) — WRITTEN, NEVER RUN in
#     this loop; the human gate runs them first:
python apps/worker/python/marl_worker/evaluation_harness.py
python apps/worker/python/marl_worker/shadow_log.py
python apps/worker/python/marl_worker/safety_filter.py
python apps/worker/python/marl_worker/marl_worker.py

# (b) the pytest suites (incl. the new evaluation-harness mirror and the
#     P8.14 boundary tests):
PYTHONPATH=apps/worker/python/marl_worker python -m pytest \
  apps/worker/python/marl_worker/tests -v

# (c) the boundary inspection (P8.14 §6 carried forward — the new module
#     must not change the answer):
grep -rn "grpc\|socket\|requests\|http\|pandapower\|helics\|import andes" \
  apps/worker/python/marl_worker --include=*.py
#     expected: no hits outside comments

# (d) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (e) FIRST RUN — the fills of §2.1/§4/§5/§6 (human/lab inputs, in the
#     recorded isolated research environment):
#     1. resolve the framework DECISION-POINT (P8.13 §5) and obtain the
#        CBF derivation sign-off (P8.13 §3) — training stays blocked
#        until both exist;
#     2. freeze the P8.06 baseline results per case (same cases, same
#        accounting) and pre-register the tolerances BEFORE filling the
#        verdict columns (X-8);
#     3. run the suite with the §3 procedure (P8.20 record per run;
#        P8.19 benchmark-set version cited per row);
#     4. record each decision's consequence via build_case_consequence
#        (consequence_status -> recorded_by_harness);
#     5. assemble §4/§5 from the harness outputs only; freeze the run
#        records (P8.20 artifact store);
#     6. independent re-run of two cases from the recorded seeds/
#        artifacts (the ledger's verification method) and confirm
#        reproduction within the P8.38 pre-recorded tolerance.
```

A green (a)–(d) earns only a one-line "human build/test gate: PASSED
(date)" appended here; failures are recorded verbatim and the ledger tick
reverts to `- [ ]`. The measured cells of §4/§5/§6 stay EMPTY until the
(e) runs fill them.

## 9. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The PI baseline has no frozen id (P8.06).** §13.3 names
   "rule-based/PI baselines"; the frozen P8.06 registry contains only the
   three tiers. A PI baseline must be defined, run, and frozen in P8.06
   before the delay/loss/uncertainty cases can be scored against it; the
   harness refuses such a comparison until then (`BASELINE_UNKNOWN`).
   Never invented here.
2. **The P8.14 log schema does not carry the fallback facts on the
   `infeasibility` object** (`fallback_attempted` / `fallback_viable`
   live only in the filter report). An ADDITIVE log-schema extension
   (new optional keys on the existing object, schema version bump) is the
   recorded follow-up; the harness reports
   `NOT_RECORDED_IN_LOG_SCHEMA` meanwhile — never a fabricated default.
3. **The shadow-log schema carries no case-id field** — the per-case
   log slice is a caller decision. An additive `case_id` field (with the
   P8.35/P8.20 provenance linkage) is the recorded follow-up; until then
   the harness requires the caller to inject exactly one case's entries
   and the record documents the slicing.
4. **The P8.06 frozen RESULTS artifact does not exist** (§3 of
   `05_06_harness_baselines.md` recorded this): the baseline comparison
   columns cannot be filled until the first human/lab run freezes the
   baseline results per case. No gap fix is invented here.
5. **The environment path (framework + simulator) that produces
   consequences is UNREACHABLE in this repository** (no framework
   installed; the P8.13 DECISION-POINTs open) — the consequence-record
   contract declared here is the P8.15 fill-in surface, an additive
   specification over the P8.14 log schema, not a parallel contract.

## 10. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| All evaluation cases run and reported | **UNRUN** — template with EMPTY cells (§2.1/§4/§5); runs are the human/lab gate |
| Baselines compared on the same cases | Rules and refusal discipline recorded (§2.2/§4); the comparison requires the P8.06 frozen results artifact (not yet in existence — §9.4) |
| Filter statistics complete | Aggregation code written with hand-computed self-tests (§5); the outcome of EVERY infeasibility is listed per occurrence; the rates are EMPTY until a run exists |
| The evaluation note contains the finite-test-set limitation in plain language | DONE — §6 item 1, written BEFORE any run, verbatim per §13.3/X-4 |

## 11. Non-claims (restated)

No safety guarantee from the finite test set (§6 item 1); no field
authority (proposals only, ADR-010; X-2); no verdict without a
pre-registered threshold (X-8); no robustness claim beyond the tested
shift families; no merged headline metric; no filter-statistics rate from
a real run exists anywhere in this record or its code — the self-tests'
toy fractions pin the definitions and are not measurements.
