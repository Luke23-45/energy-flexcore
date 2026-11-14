<!-- P9.38 — Pilot shadow replay: §24 step 10 at the pilot (decision record). TEMPLATE ONLY: every run-record cell EMPTY, status UNRUN. No shadow data exists (P9.17 NOT EXECUTED); no tolerance VALUE is pre-registered (X-8: values are registered BEFORE the run, never fitted after); the comparison rules below pre-register the CLASSES and the procedure, not numbers. -->
# P9.38 — Pilot Shadow Replay: §24 Step 10 at the Pilot (Template)

Date: 2026-11-14. Subtask: P9.38 (`ledger_phase_9.md` §P9.38).
Owner role: Integration engineering.
Report trace: §23 Stage 9; §24 step 10; §19.2(5) (deterministic scenario
tests); §21.4 (read-only/shadow).
Inputs: P9.17 shadow-operation records (NOT EXECUTED — do not exist);
P9.18 telemetry verification (NOT EXECUTED); frozen versions per P9.16 (NOT
RECORDED — the subtask has not run). **None of the inputs exists.**

## 1. Honest status — the plain statement

**No pilot shadow data exists, so no shadow replay has been run or can be
run, and none is claimed.** This record delivers what an agent can deliver
under ENTRY_NOTE boundary 6: the §24-step-10 structure applied at the pilot
(§2), the comparison RULES pre-registered BEFORE any run — the tolerance
CLASSES and the anti-fitting procedure, with every tolerance VALUE cell
EMPTY per X-8 (§3), the data-preparation procedure naming exactly what
pilot data must exist and does not (§4), and the run record as an EMPTY
template (§6) whose fill is the human gate. The replay itself, the
divergence analysis, and the verdict are human executions on real recorded
data; nothing here stands in for them.

The ledger's named failure modes are structural rules of this template, not
cautionary prose: a replay that REUSES the original proposed actions instead
of RECOMPUTING them from stored inputs is not a replay and fails §6's
procedural check by construction; a tolerance fitted after seeing a
divergence is forbidden by §3's pre-registration rule and voids the record.

## 2. The §24 step 10 structure at the pilot

§24 step 10 (report trace) is the replay proof: re-execute a recorded
period from its stored inputs alone and compare the recomputed outcomes
against the recorded originals. Applied at the pilot:

1. **Select** a recorded SHADOW period (§21.4 read-only/shadow — no field
   write authority exists during it): at least one full operating day, or
   the longest continuous shadow run if shorter (ledger step 1).
2. **Freeze** the period's stored inputs: telemetry snapshots (with their
   canonical quality states), grid-model version, schedule inputs, seeds (if
   any randomness is configured), and the software versions (P9.16 frozen
   set) — everything the platform consumed during the original run.
3. **Replay** the period on the SAME frozen versions from the stored inputs
   ALONE: no new telemetry, no live feeds, no re-connection to the site, no
   seed override. Every input that cannot be reproduced from the record is
   an ERROR, never a silent default — the same replay rule the pinned-record
   machinery enforces (`apps/sim-runner/src/replay_record.rs`:
   `verify_pinned_inputs` fails loud on any identity/clock/seed mismatch;
   `scenario_json_with_seed` is deliberately NOT on the replay path).
4. **Compare** the replayed PROPOSED actions (schedules, command
   proposals, accept/reject decisions — everything the platform WOULD have
   proposed) against the recorded originals, within the tolerance recorded
   BEFORE the replay (§3).
5. **Record** every beyond-tolerance divergence WITH its cause
   (nondeterminism source, input difference, version difference) — ledger
   step 3. A divergence with no recorded cause is an OPEN FINDING, not a
   rounding note.
6. **File** the replay record with the field-acceptance evidence (P9.17) —
   ledger step 4.

Control status of the replay itself: it re-executes a SHADOW period on
frozen versions in a controlled environment; it is simulation-of-record,
never field execution. The replay grants no field write authority and
proves nothing about live-grid behaviour (§7).

## 3. Comparison rules — pre-registered BEFORE the run (X-8; the P8.38 standard)

The class/value distinction is the P8.38 disclosure standard's
(`ledger/phase-08-advanced-coordination/38_gpu_nondeterminism.md` §1, applied
here by citation): this section fixes WHICH tolerance CLASS applies to each
comparison kind — a structural fact declarable now — and does NOT fix any
tolerance VALUE. X-8 (`ledger/phase-08-advanced-coordination/02_research_scope_freeze.md`,
no arbitrary universal targets) governs the values: each is registered in
the pre-registration entry BEFORE the replay, with its basis (the metric's
decision relevance and the observed run-to-run behaviour of the frozen
build — never a universal target), and never edited afterwards (P8.38 §3.2
rule 1 mirrored; a change is a NEW pre-registration version and the affected
comparisons are re-judged under the class they ran).

### 3.1 The class assignments (declared NOW)

| Comparison | Class | Basis (recorded) |
|---|---|---|
| Discrete outcomes: command-lifecycle transitions (§4 vocabulary), accept/reject/refuse decisions, dispatch issue/no-issue, per-point canonical quality states (the frozen six, P0.08) | `exact` | a proposed action either reproduces or it does not; there is no "approximately rejected" |
| Numeric setpoints: schedule MW/MVAr values per interval, target P/Q of a proposed command, SOC/energy-derived figures | `within_tolerance` | floating-point recomputation of a scheduling pipeline over stored inputs is expected to agree to within a registered numeric tolerance, not bit-for-bit, when any input timestamp/registration ordering differs; the VALUE is registered before the run |
| Input identity: the stored-input inventory of §4 (file hashes, versions, seeds, grid-model reference) | `exact` | replay uses ONLY recorded inputs; any mismatch is `INVALID_RERUN_IDENTITY`, not a deviation |
| Timing: cadence grid alignment (the scenario/scheduling clock, never a wall clock, P0.09 §5) | `exact` | the replay clock is derived from the stored inputs; a timing shift is an input-identity finding |

The sim-runner's recorded numeric defaults (relative 1e-9, absolute floor
1e-12 — `apps/sim-runner/src/replay_record.rs` `RecordTolerances` and
`apps/sim-runner/src/reproducibility.rs`) are the SCRIPTED-IN-PROCESS
simulator-leg defaults, cited for machinery continuity. They are NOT
pre-registered pilot values and must not be quoted as one: the pilot
tolerance value for the numeric class is registered per §3.2 with its own
recorded basis.

### 3.2 The pre-registration procedure (BEFORE the replay — X-8)

Before the replay executes, the human executor records one pre-registration
entry per compared outcome kind: `{outcome_kind, tolerance_class,
tolerance_value (for within_tolerance), basis, registered_utc,
registered_by, reference}`. Rules:

1. The entry is written and dated BEFORE the replay; it is never edited
   after — a change is a new pre-registration version.
2. The basis states why the number, in terms of the pilot's own decision
   risk — never a universal or fleet-wide target (X-8).
3. The entry is referenced by the run record (§6, `pre_registration_ref`);
   a replay record whose compared kinds lack a pre-registration entry is
   INVALID (`TOLERANCE_NOT_PRE_REGISTERED` — the P8.20/P8.38 comparator
   refusal, mirrored here).
4. Every VALUE cell in this record is EMPTY — writing numbers here, with no
   pilot, no shadow data, and no frozen build, would fabricate the X-8
   precondition.

### 3.3 The verdict vocabulary (P8.38 §8 by citation)

The replay verdict uses exactly the P8.20/P8.38 comparator vocabulary:
`REPRODUCED_EXACT`, `REPRODUCED_WITHIN_PRE_REGISTERED_TOLERANCE`,
`NOT_REPRODUCED_MARKED_FOR_EXCLUSION`, `INVALID_RERUN_IDENTITY`,
`BLOCKED_*`. A replay that does not reproduce within its pre-registered
tolerance carries `NOT_REPRODUCED_MARKED_FOR_EXCLUSION` AS THE OUTCOME —
recorded, never silently retried into significance, never reworded as a
rounding note. (The sim-runner's own leg verdicts — `identical` /
`within_tolerance` / `mismatch` per leg — remain the machinery-level
rendering; the pilot record's verdict is the P8.38 vocabulary on top of
them.)

### 3.4 Divergence handling (ledger step 3)

Every beyond-tolerance divergence is recorded with its cause from the closed
set: {nondeterminism source (named, with the nondeterminism-controls
inventory of the frozen build), input difference (named input + both
values), version difference (named component + both versions)}. A divergence
with no assignable cause is an OPEN FINDING carried to P9.27 — never a
tolerance adjustment.

## 4. Data-preparation procedure — what must exist (none does)

Every artefact below is REQUIRED before the replay can be scheduled. None
exists today; each is produced by a named upstream subtask, never by this
record.

| # | Required artefact | Producer | Status |
|---|---|---|---|
| D-1 | A recorded shadow period ≥ 1 full operating day (or the longest continuous shadow run if shorter), with start/end instants and the site id | P9.17 shadow operation | NONE EXISTS — P9.17 NOT EXECUTED |
| D-2 | Stored input telemetry snapshots for the period (canonical records with the six quality states; the stored-input hashes) | P9.17 + P9.18 telemetry verification | NONE EXISTS |
| D-3 | Grid-model version reference consumed during the original run | P9.05/P9.03 outputs, as recorded by P9.17 | NONE EXISTS |
| D-4 | Schedule inputs for the period (forecasts/baselines/constraints as consumed) | P9.17 records | NONE EXISTS |
| D-5 | Seeds (only if any configured randomness was in effect; the derivation travels with the seed) | P9.17 records | NONE EXISTS |
| D-6 | Frozen software versions (the P9.16 frozen set: platform build, edge build, worker versions) | P9.16 | NONE EXISTS — NOT RECORDED |
| D-7 | The recorded originals to compare against: proposed schedules, command proposals, accept/reject decisions, lifecycle transitions from the shadow run | P9.17 records | NONE EXISTS |
| D-8 | The §3.2 pre-registration entries (classes from §3.1, VALUES + basis) | human executor, BEFORE the replay | NONE EXISTS |
| D-9 | The replay environment spec (host, OS, deployment composition identical to the shadow deployment's frozen versions) | human executor | NONE EXISTS |

Preparation rule: the replay reads the stored inputs ALONE. Recomputing
proposed actions requires recomputation from inputs, never replaying
outputs: any comparison whose "replayed" side was read from the original
run's output records (rather than recomputed) is void — the ledger's named
failure mode, checked explicitly in §8 (reviewer check 4).

## 5. Machinery status — what exists and what blocks a pilot replay today

What exists (WRITTEN, and exercised only in Phase 2–5 lab shape): the
pinned-record replay machinery — `apps/sim-runner/src/replay_record.rs`
(ScenarioRecord v3: pinned inputs, input hash, fail-loud pin verification,
pre-registered expectations slot, thread_count 1, tolerance pins),
`apps/sim-runner/src/replay.rs` (the canonical P2.24 comparison tuple with
`ingest_time` excluded by record shape), `apps/sim-runner/src/compare.rs`
(the frozen tuple comparator), `apps/sim-runner/src/scenario_run.rs` (the
§24-step harness), and the `replay-run` CLI (`--double-run` /
`--baseline`).

What BLOCKS a pilot replay on the existing CLI (a recorded contract gap,
not silently worked around — see §9 follow-up 1): the ScenarioRecord v3
validation refuses non-synthetic records (`synthetic: false` is a named
error requiring a named real model with confirmed licensing/availability).
A pilot shadow-period record is NOT a synthetic scenario — it is recorded
field-adjacent data. The pilot replay therefore executes through the
platform deployment composition from stored inputs per §2, with the
comparison rendered per §3/§6; the ScenarioRecord v3 schema extension for
recorded (non-synthetic) shadow-period inputs is a Phase 7 follow-up (§9).
Until that seam exists, no `replay-run` invocation against pilot data is
possible, and none is claimed.

## 6. Run record — EMPTY template (the human gate input)

**EVERY cell EMPTY. No replay has been scheduled, executed, or witnessed.
Filling a cell from anything other than the actual replay output is
fabrication (AGENTS.md rule 1).**

### 6.1 Period identity and inputs

| Field | Value (EMPTY — fill at the run) |
|---|---|
| Pilot site id (signed P9.01 freeze) | |
| Shadow period start/end (UTC, with the selection basis: full day / longest continuous run) | |
| Stored-input inventory reference (file/commit; hashes per D-2/D-3/D-4/D-5) | |
| Input-hash check result (recomputed vs recorded; any mismatch = `INVALID_RERUN_IDENTITY`) | |
| Frozen versions (P9.16 set: platform, edge, workers; equality check vs the original run) | |
| Grid-model version | |
| Seeds (or "none configured", recorded) | |
| Pre-registration reference (§3.2 entries, with registered_utc/by) | |
| Replay environment (host, OS, deployment composition) | |
| Executor / second-engineer witness | |

### 6.2 Comparison table (one row per compared proposed action / outcome kind)

| Outcome kind | Class | Pre-registered value | Recorded original | Replayed (recomputed) | Deviation | Verdict (P8.38 vocabulary) |
|---|---|---|---|---|---|---|
| e.g. schedule setpoint, interval `<t>` (illustrative ROW STRUCTURE only — no data exists) | within_tolerance | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY — UNRUN) |
| e.g. command proposal accept/reject | exact | n/a | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY — UNRUN) |

### 6.3 Divergence table (ledger step 3)

| Divergence | Class | Cause (nondeterminism source / input difference / version difference — named) | Disposition |
|---|---|---|---|
| (none — exists only when an actual beyond-tolerance divergence is recorded) | | | |

A divergence with no recorded cause is an OPEN FINDING carried to P9.27
(§3.4) — the row is filled, not deleted.

### 6.4 Verdict

| Field | Value (EMPTY — fill at the run) |
|---|---|
| Overall verdict (REPRODUCED_EXACT / REPRODUCED_WITHIN_PRE_REGISTERED_TOLERANCE / NOT_REPRODUCED_MARKED_FOR_EXCLUSION / INVALID_RERUN_IDENTITY / BLOCKED_*) | (EMPTY — UNRUN) |
| Verdict statement (what this verdict means per §3.3) | (EMPTY) |
| Filed-with reference (the P9.17 field-acceptance evidence, ledger step 4) | (EMPTY) |

## 7. What a passing replay would and would not establish

A passing replay (`REPRODUCED_EXACT` or
`REPRODUCED_WITHIN_PRE_REGISTERED_TOLERANCE` on the pre-registered classes)
would establish ONLY: that the pilot platform, on the frozen P9.16 versions,
recomputed the same PROPOSED actions from the recorded inputs alone, within
the tolerance pre-registered BEFORE the replay, for THAT recorded period —
the §24 step 10 replay proof at the pilot, evidencing the determinism of
the scheduling pipeline on recorded data under shadow (§21.4) conditions.

It would NOT establish any of the following, and a filled record may claim
none of them: correctness of the platform against FUTURE or unrecorded
conditions (a finite replay covers the recorded period only — a finite test
set is not a guarantee, the X-4 discipline); safety of any proposed action
or of field execution (AGENTS.md §3 rule 2, verbatim: "FlexCore is not
certified, not a safety case, and not permission for live grid control");
any protection-grade or sub-cycle capability (rule 3 — FlexCore is
supervisory orchestration only); forecast quality or schedule optimality
(the replay compares recomputation against recomputation, not proposals
against ground truth); readiness for bounded automation or any control
beyond the pilot's recorded scope (bounded automation is post-1.0 and
excluded from the pilot, ENTRY_NOTE boundary 3); reproducibility of a
different period, site, or version set. A failing replay is recorded AS THE
OUTCOME with the divergence causes (§6.3) — it does not invalidate the
platform, it invalidates the reproduction claim for that period until the
causes are dispositioned.

## 8. Reviewer-verification procedure

1. Confirm §1's plain statement is unchanged and no executed-replay claim
   exists anywhere in the record.
2. Confirm every §3.2 VALUE cell, every §4 Status cell, and every §6 cell is
   EMPTY / `(EMPTY — UNRUN)` / `NONE EXISTS` as applicable.
3. Confirm §3's class assignments carry bases, not values, and that the
   sim-runner numeric defaults are cited as machinery defaults, never as
   pre-registered pilot values (§3.1).
4. Anti-fitting check (for a filled future record): the pre-registration
   entry predates the run (timestamps); no tolerance value was edited after
   the replay it judged; the replayed side was RECOMPUTED from stored inputs
   (trace the invocation and its input hashes — an output-reuse replay is
   void, §4 preparation rule); every beyond-tolerance divergence has a named
   cause or is carried as an open finding.
5. Second-engineer reproduction (the ledger's verification method): a
   second engineer replays the SAME period from the run record ALONE — the
   stored inputs, the frozen versions, the pre-registration entries, and the
   procedure of §2 — and confirms the comparison outcome independently.
6. Confirm §7's AGENTS.md §3 quotes are verbatim and the report trace
   (§24 step 10; §19.2(5); §21.4) resolves to real headings.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; no replay has been executed.

## 9. Filling commands (for the human executor)

```sh
# (a) Machinery-side proof (synthetic lab shape ONLY — never pilot data;
#     see §5: the pilot replay executes through the platform composition,
#     not this CLI, until the §9 follow-up 1 seam exists):
cargo run -p flexcore-sim-runner -- replay-run --record <scenario-record>.json --double-run
cargo run -p flexcore-sim-runner -- replay-run --record <scenario-record>.json --baseline <stored-run-record>.json

# (b) THE PILOT SHADOW REPLAY (after D-1..D-9 exist and the §3.2 entries are
#     registered): execute the replay per §2 through the pilot deployment
#     composition from the stored inputs alone, on the frozen versions;
#     render the comparison per §3/§6; the exact invocation is recorded IN
#     the run record (§6.1) at execution time. It is NOT pre-writable here
#     because the deployment composition does not exist yet — writing it now
#     would fabricate the environment.

# (c) The documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

No cargo command has been executed in this writing loop (banned); every run
above is a human gate input.

## 10. Phase 7 follow-ups

1. **ScenarioRecord schema for recorded shadow periods:** extend the
   ScenarioRecord v3 lineage (or define its additive successor) to carry a
   NON-synthetic recorded-input record — the stored telemetry snapshots,
   grid-model reference, schedule inputs, seeds, and versions of §4 — so the
   existing fail-loud pin verification, input hash, tolerance pins, and
   comparison legs run unchanged against pilot replay records. This is the
   Phase 7 seam owners' contract change; it is never worked around by
   marking pilot data `synthetic: true` (that flag means something and must
   stay honest).
2. **Proposal-action capture contract:** the run record (§6.2) compares
   PROPOSED actions; the recording of proposed schedules/decisions during
   shadow operation is P9.17's deliverable, and its persistence format
   (what the replay consumes as "recorded originals") must exist as a
   stable, versioned contract — owned with the Phase 7 seam owners, not
   invented inside this template.
3. **Second-engineer replay tooling:** the §8 check-5 reproduction needs the
   replay procedure to be executable from the record alone; whatever runner
   materializes for follow-up 1 must support that (the P8.20 second-engineer
   rule mirrored), including the `INVALID_RERUN_IDENTITY` input-hash check.

## 11. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No shadow data exists (P9.17 NOT EXECUTED): the §24 step 10 proof cannot start for the whole pilot period | §1, §4 |
| Replay reuses original outputs instead of recomputing them — the void-replay failure mode | ledger P9.38 risk row; §4 preparation rule; §8 check 4 |
| Tolerance fitted after seeing a divergence — voids the record | ledger P9.38 risk row; §3.2 rules |
| A beyond-tolerance divergence with no assignable cause remains an open finding against the pipeline's determinism | §3.4, §6.3 |
| ScenarioRecord v3 refuses non-synthetic records: the existing replay CLI cannot execute a pilot replay until the §9 follow-up 1 seam exists | §5 |
| Version drift between the original shadow run and the replay (P9.16 freeze missing or stale) — a version difference masquerading as nondeterminism | §3.4 closed cause set; D-6 |
| "Passing" replay over-read as field-readiness or safety evidence | §7 |

Status: **WRITTEN-UNRUN** — a shadow replay cannot be produced by an agent
in this loop; no pilot, no shadow data, no frozen version set, and no
pre-registered tolerance value exist; every run-record cell is EMPTY and
stays EMPTY until the human execution fills it from actual replay output.
Nothing was run, and no reproduction is claimed.
