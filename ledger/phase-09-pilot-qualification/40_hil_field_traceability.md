# P9.40 — HIL-to-field traceability map: the MAP RULES + versioned EMPTY matrix artifact (WRITTEN-UNRUN — no HIL case, no FAT case, no coverage exists)

Date: 2026-11-14. Subtask: P9.40 (`ledger_phase_9.md` §P9.40). Owner role:
integration engineering (with power systems). Report trace: §23 Stage 9;
§19.1 (the HIL track — CHIL vs PHIL distinct); §19.2(10) (HIL/commissioning
tests run ONLY after simulation, integration, and cyber reviews pass);
`RESEARCH.md` §4 (the validation pathway digital-twin/simulation → HIL/PHIL
→ field, **with the same disturbance profiles and seeds at each stage so
evidence is comparable**). Inputs per the ledger: **P9.15** HIL report
(the concurrent wave-5 deliverable — referenced by path, never edited by
this subtask), **P9.14** lab report, **P9.16** FAT plan. Artifact-freeze
precedent: `benchmarks/research-set-v1.json` (the P8.19 versioned-artifact
discipline this map mirrors: deterministic field order, a change is a NEW
version frozen as a NEW file, never edited in place after freeze).

## 0. Run-status honesty — READ FIRST

**No HIL run has happened, no field acceptance test exists, and no coverage
exists.** The upstream plans this map traces between are themselves
templates: P9.15 (the HIL test plan and execution) is the concurrent
wave-5 subtask whose execution is a human gate, and P9.16 (the field
acceptance test plan) is the wave-6 deliverable, also a template until its
human gate. **Every matrix row and every coverage-summary cell is
deliberately EMPTY** — a coverage count here would be a fabricated figure
(ENTRY_NOTE.md boundary 1: never fabricate a measurement or a sign-off).
What this subtask delivers is the map's SCHEMA, its RULES (§3 — the
normative content), the versioned artifact that will hold the matrix
(`hil-field-traceability-map-v1.json`, frozen WRITTEN-UNRUN), and the
filling procedure (§6). The "same disturbance profiles and seeds at each
stage" statements in §3 cite `RESEARCH.md` §4 as recorded — they are
requirements this map ENFORCES, not evidence that any stage reused
anything.

## 1. What the map is

The ledger step 1, verbatim intent: build the map
**FAT case → HIL case(s) → lab case(s) → simulation case(s)**, with the
disturbance profile and seed used at each stage — so that no
field-acceptance test case rests on no upstream evidence. The ledger's
reason for the map is §19.2(10)'s ordering discipline and `RESEARCH.md` §4's
comparability rule: HIL evidence de-risks a field test only if the same
disturbances were exercised, with the same parameters, on the way up the
fidelity ladder.

Two directions, both required:

1. **Forward (per FAT case):** the field acceptance case, the HIL/lab/
   simulation cases that de-risk it, the evidence each upstream stage was
   required to produce, the disturbance profile and seed per stage, and
   the coverage status (mapped, or a recorded gap).
2. **Reverse (per HIL case):** the HIL case, the FAT case(s) its evidence
   de-risks, and whether the profile/seed it ran is the one the FAT stage
   will run — the direction a reviewer walks when asking "what does this
   HIL pass actually justify?".

Both directions live in ONE artifact so they cannot drift apart: the
forward matrix is the acceptance surface (ledger criterion (a)); the
reverse matrix is the reviewer surface (the ledger's verification method —
pick FAT cases and trace to evidence; the reverse rows make that walk
mechanical).

## 2. The map artifact — versioned, additive, deterministic

**Artifact:** `ledger/phase-09-pilot-qualification/hil-field-traceability-map-v1.json`
(NEW this subtask; `version_id` =
`flexcore-pilot-hil-field-traceability-map-v1`; `status` = WRITTEN-UNRUN;
`frozen_date` = 2026-11-14).

The ledger says "File the map with the FAT plan (P9.16) before P9.17
executes" — the map is therefore a FILE, not prose, and it gets the frozen-
artifact discipline because a map that can be silently edited is exactly
the back-fitting risk the ledger names (§3 R-6):

| Property | Rule |
|---|---|
| Versioning | A change to ANY entry — a case added/removed/re-mapped, a profile or seed changed, a version pin changed — is a NEW `version_id` frozen as a NEW file (`…-v2`, …). The v1 file is never edited in place after freeze (the `research-set-v1.json` `schema_note` discipline, verbatim pattern) |
| Deterministic order | Field order in the artifact is NORMATIVE; rows are ordered by case id ascending (lexicographic); reviewers diff versions textually |
| Row sourcing | Every non-EMPTY cell is filled FROM the named source records by case id (R-9): the P9.14 record, the P9.15 record, the P9.16 record, the P9.17 record — never from memory, never an invented case id |
| No growth without sources | A matrix row whose `fat_case_id` does not exist in the cited P9.16 record is a named finding against the map, never absorbed |
| Referenced inputs | The artifact references the P9.13–P9.17 records BY PATH (`inputs_by_path`) and edits none of them — this subtask's file ownership is the record (this file) + the map artifact + the ledger tick only |

**No `benchmarks/` artifact is added by this subtask.** The map is a
pilot-qualification artifact, not a research benchmark set; placing it in
the frozen benchmark directory would grow that directory outside its own
anti-silent-growth rule (the P8.20 precedent of NOT adding artifacts a
ledger step does not dictate).

## 3. The map RULES (normative — every filling act is subject to them)

| Rule | Statement | Source of the rule |
|---|---|---|
| **R-1 — a gap is a gap** | A FAT case with no upstream row is recorded `UNMAPPED-GAP-OPEN`, never "covered". Its disposition is exactly one of: (a) the upstream evidence is added (the HIL/lab case is defined and run; a new map version is frozen), or (b) a signed CUSTOMER acceptance of the gap is filed in **P9.27**. Never left blank, never waved through. | ledger step 1 ("recorded as unmapped, not as covered") and step 3 |
| **R-2 — HIL justifies execution, never the field RESULT** | A HIL (or lab, or simulation) pass justifies the corresponding field test's EXECUTION — that running it is justified and its expected behaviour is pre-declared. It never justifies, substitutes for, or predicts the field RESULT: the field result exists only after the field run (P9.17) at the site, in the recorded control-authority mode (§21.4). A FAT case is never marked "passed" because its HIL row passed. | §19.1 (HIL is a simulated-grid track); ENTRY_NOTE boundary 1 (field tests are human gate inputs); the ledger's objective ("de-risks") |
| **R-3 — profiles and seeds reused across stages, parameters recorded** | The same disturbance profiles and seeds are reused simulation → lab → HIL → field WHERE THE STAGE SUPPORTS THEM (`RESEARCH.md` §4), and each row records the ACTUAL parameters in force per stage (profile id; amplitude/duration/cadence as applicable; seed + its derivation string as applicable) — not just the profile name. Where a stage could not reuse a profile or seed, the row records the stage, the reason, and what was used instead. A profile reused by name while its parameters differ between stages is the ledger's named failure mode. | ledger step 2; RESEARCH.md §4; the ledger risk list |
| **R-4 — version consistency** | HIL evidence on equipment/firmware versions differing from the pilot's cannot justify the FAT case. Rows carry the versions in force per stage; a mismatch is `BLOCKED-version-mismatch` with an R-1 disposition (re-run on the pilot versions, or a signed accepted gap in P9.27). | P9.15 acceptance criterion (c) |
| **R-5 — CHIL vs PHIL honesty** | Coverage records the HIL class actually run — CHIL or PHIL — per the P9.15 scope record. A CHIL pass is never recorded as PHIL coverage (PHIL additionally requires a power amplifier and an interface algorithm with its stability condition, RESEARCH.md §4). | P9.15 step 1; RESEARCH.md §4 |
| **R-6 — filed before execution** | The map is filed with the FAT plan (P9.16) BEFORE P9.17 executes. The `version_id` + `frozen_date` are the ordering evidence. A map built after FAT execution and back-fitted to the results is the ledger's named failure mode; if execution ever precedes a map version, that is a recorded finding against the deployment, never repaired by re-dating. | ledger step 3; ledger risk list |
| **R-7 — pre-gate evidence required** | A HIL row is enterable as coverage only where the P9.15 record files the §19.2(10) pre-gate evidence (simulation, integration, and cyber reviews passed). A HIL case without its pre-gate evidence is not coverage; it is a gap. | §19.2 item 10; P9.15 step 4 |
| **R-8 — no safety/certification claim** | The map is an integration-de-risking record. HIL coverage never implies equipment conformance or certification (IEEE 1547.1-2020 conformance testing is a separate responsibility, §11.3; RESEARCH.md §4 implication), never a safety case, never protection-grade capability, and never field write authority — the pilot is operator-approved control AT MOST (§21.4; ENTRY_NOTE boundary 3). | AGENTS.md §3; ENTRY_NOTE boundaries 3–4 |
| **R-9 — sourcing discipline** | Every non-EMPTY cell is filled from the named source records by case id. Case ids that do not exist in the cited record are a named finding against the map; the map is re-frozen only after the source record is corrected by its owner. | the no-hallucinated-facts rule (AGENTS.md §3 rule 1) applied to the matrix |

## 4. The matrix schema (as frozen in the artifact)

**Forward matrix** (one row per FAT case, ordered by `fat_case_id`):

`fat_case_id` · `fat_case_source_record` · `justified_by_hil_case_ids` ·
`hil_class (CHIL/PHIL per case; R-5)` · `hil_evidence_required` (the
per-case evidence the HIL record must contain: observed-vs-expected
behaviour, equipment/firmware versions, raw-capture reference) ·
`justified_by_lab_case_ids` · `justified_by_simulation_case_ids` ·
`disturbance_profile_per_stage` (profile id + ACTUAL parameters per stage;
reuse exceptions with reason, R-3) · `seed_per_stage` (R-3) ·
`versions_per_stage` (R-4) · `pre_gate_evidence_ref` (R-7) ·
`coverage_status` (`MAPPED` / `UNMAPPED-GAP-OPEN` /
`UNMAPPED-GAP-ACCEPTED-in-P9.27` / `BLOCKED-version-mismatch`) ·
`gap_disposition_ref` (the P9.27 entry or the re-run record; EMPTY when
MAPPED).

**Reverse matrix** (one row per HIL case, ordered by `hil_case_id`):

`hil_case_id` · `hil_class` · `hil_source_record` ·
`de_risks_fat_case_ids` · `downstream_lab_case_ids` ·
`downstream_simulation_case_ids` ·
`disturbance_profile_and_seed_shared_with_fat_stage` (yes/no + reference) ·
`coverage_status`.

**Coverage summary:** the frozen artifact carries it with every cell EMPTY
(`fat_cases_total`, `mapped_count`, `unmapped_gap_open_count`,
`unmapped_gap_accepted_in_p9_27_count`, `blocked_version_mismatch_count`,
`profiles_reused_across_all_stages_count`, `profile_reuse_exceptions_count`,
`seed_reuse_exceptions_count`, `hil_cases_without_pre_gate_evidence_count`,
`summary_valid_as_of`, `summary_author`) — filled only from the matrices,
never estimated (both plans are templates; there is nothing to count).

### Why the evidence-required column exists (the task's third leg)

The map does not stop at "HIL case X justifies FAT case Y": each row names
the EVIDENCE the upstream record must contain for the mapping to hold —
the observed-vs-expected behaviour per case, the equipment/firmware
versions under test (R-4), the raw-capture reference (the P9.15
verification method spot-checks a case by re-running it or inspecting the
raw capture), and the pre-gate reference (R-7). An upstream row whose
evidence-required cells are not satisfiable from the P9.15 record is
`UNMAPPED-GAP-OPEN`, not mapped-by-name.

## 5. Coverage summary — ALL CELLS EMPTY (no HIL case, no FAT case exists)

| Coverage item | Value |
|---|---|
| FAT cases total | EMPTY (the P9.16 plan is a template) |
| Mapped | EMPTY |
| Unmapped — gap open | EMPTY |
| Unmapped — gap accepted in P9.27 | EMPTY |
| Blocked (version mismatch) | EMPTY |
| Profiles reused across all stages | EMPTY |
| Profile-reuse exceptions | EMPTY |
| Seed-reuse exceptions | EMPTY |
| HIL cases without pre-gate evidence | EMPTY |

**Honest note:** there is nothing to count. The P9.15 HIL plan (concurrent
wave-5 deliverable) and the P9.16 FAT plan (wave 6) are both templates with
EMPTY execution cells by design (ENTRY_NOTE boundary 1). The first filled
row appears only when both records carry real case ids and the human gate
executes §6.

## 6. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the artifact's JSON validity and determinism (no new dependency;
#     std json only), run with -I as required:
python -I -c "import json;d=json.load(open('ledger/phase-09-pilot-qualification/hil-field-traceability-map-v1.json'));assert d['version_id']=='flexcore-pilot-hil-field-traceability-map-v1' and d['status']=='WRITTEN-UNRUN';assert d['forward_matrix_rows']==[] and d['reverse_matrix_rows']==[];print('map artifact v1 valid: EMPTY matrices, version and status as frozen')"

# (c) FILLING (human gate, after P9.14/P9.15 records exist and the P9.16
#     plan carries real case ids):
#     1. copy each FAT case id from the P9.16 record into a forward row;
#        copy each HIL case id from the P9.15 record into a reverse row;
#     2. fill the mapping cells FROM the records (R-9) — profile id and
#        ACTUAL parameters per stage, seed + derivation per stage (R-3),
#        versions per stage (R-4), CHIL/PHIL class (R-5), pre-gate ref
#        (R-7), evidence-required per case;
#     3. every FAT case with no upstream row gets coverage_status
#        UNMAPPED-GAP-OPEN and an R-1 disposition (re-run the HIL/lab
#        case, or file the signed customer gap acceptance in P9.27);
#     4. freeze the filled map as a NEW version file (...-v2, ...) with a
#        new frozen_date; never edit v1 in place;
#     5. re-run (a) and re-run (b) against the new version id.

# (d) THE LEDGER'S VERIFICATION METHOD (independent reviewer): pick THREE
#     FAT cases and trace them to their HIL/lab evidence through the map;
#     confirm the profiles/seeds match ACROSS STAGES BY PARAMETER, not by
#     name (R-3), and that the versions match the pilot's (R-4). Record
#     the reviewer identity and date in the coverage summary's
#     summary_author / summary_valid_as_of cells of the frozen version.

# (e) ORDERING CHECK before P9.17 starts: the map version filed with the
#     P9.16 plan predates the first P9.17 execution record (R-6); the
#     frozen_dates are the evidence.
```

A green (a)–(b) is the current state (template + EMPTY artifact). The
ledger tick stays at "implemented, awaiting human build/test" until (c)
has produced a filled frozen version and (d) has recorded its reviewer.

## 7. Open risks carried to P9.27 (the residual-risk register input)

| # | Risk | Why it is open |
|---|---|---|
| R-1 | **The map is EMPTY because both upstream plans are templates** — until P9.15's HIL cases and P9.16's FAT cases exist with real ids, no coverage claim of any kind is possible, and the pilot's FAT execution must not start before the filled map is filed (R-6) | Recorded here; the ordering gate is human-enforced at P9.17 entry |
| R-2 | **Profile/seed reuse is a requirement, not a fact**: `RESEARCH.md` §4 records the practice; whether the pilot's HIL platform and the field stage can both express the same disturbances (sampling rate, amplitude resolution, seed applicability) is unknown until P9.15 selects the platform — the exceptions column exists because reuse will NOT be total | R-3 exceptions recorded per row at fill time |
| R-3 | **CHIL-only coverage may leave PHIL-requiring FAT cases unmapped**: if the pilot's lab supports CHIL only, FAT cases whose justification needs PHIL (real inverter behaviour) are gaps with an R-1 disposition — a signed customer acceptance in P9.27 would be recording a REAL residual risk, not paperwork | Depends on P9.15's recorded CHIL/PHIL scope |
| R-4 | **Version drift between stages** (HIL on lab firmware vs pilot firmware): P9.15's own named risk; the map makes it visible per row (R-4) but cannot prevent it | Checked at fill time and at the (d) reviewer walk |
| R-5 | **A back-fitted map** (the ledger's named failure mode): the version discipline (R-6) provides the ordering evidence, but if FAT execution is scheduled before the filled map exists, the discipline records a finding instead of preventing the mistake | The §6(e) ordering check is the enforcement point |
| R-6 | **HIL pass over-read**: the strongest failure pressure on this artifact is treating a HIL row as a field result (R-2). The rule is recorded in the artifact itself (`map_rules`), which travels with every future version | Standing rule; reviewer walks reinforce it |

## 8. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| `python -I` artifact validity check (§6 step b) | RUN in the writing loop (output quoted in the completion report) |
| The map's matrices and coverage summary | **EMPTY BY DESIGN — WRITTEN-UNRUN; no HIL case, no FAT case, no coverage exists** |
| Any HIL run, field test, or gap acceptance | **NONE EXISTS — nothing run, nothing fabricated** |
