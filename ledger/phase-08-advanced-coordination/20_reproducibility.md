# P8.20 — Reproducibility: seeds, versions, artifacts, and the rerun test — the reproducibility standard (TEMPLATE — every rerun cell EMPTY, no rerun has happened)

Date: 2026-11-14. Subtask: P8.20 (`ledger_phase_8.md` §P8.20). Owner role:
research lead. Report trace: §19.2(5); §9.2; §6.5. Inputs: **P8.04**
(`04_worker_contracts.md` — the reused worker contracts this standard
builds on: the §9.2 `ResultProvenance` base — worker/model/solver version,
input snapshot id, termination status — the `DeterminismMetadata`
(seed / request_id / idempotency_key) and the P7.42 seed-honesty
discipline); **P8.19** (`19_benchmark_cases.md` +
`benchmarks/research-set-v1.json` — the frozen benchmark set whose
`seeds.derivation` string and solver pins this standard records verbatim);
P8.06 (`05_06_harness_baselines.md` — the frozen baseline registry and its
frozen-version discipline, mirrored onto the record schema); P8.38 (the
owner of the per-module tolerance pre-registrations — PENDING, see §8);
ENTRY_NOTE.md boundary 4.

## 1. Status of this record — WRITTEN-UNRUN

This record is the **reproducibility standard** (ledger steps 1–2), the
**rerun-test definition** (step 3), and the EMPTY rerun-record template
(step 4). **No rerun has happened and no reproducibility record exists
anywhere in the repository** — every measured/executed cell in §7 is
EMPTY and is filled only by the human/lab gate via §9's commands.

What exists and is verifiable to the extent the loop allows:

- the **tooling** (`apps/worker/python/reproducibility/` —
  `seed_derivation.py`, `record_schema.py`, `rerun_check.py`, plus the
  pytest mirror in `tests/`): pure stdlib functions over INJECTED data,
  each pinned by hand-computed or theorem-derived self-tests with the
  arithmetic/reasoning STATED in the test. WRITTEN, NEVER RUN;
- the **record format** (§3): the versioned schema
  `flexcore-research/reproducibility-record.v1`, validated by the
  tooling, named refusals for every missing or placeholder-bearing field;
- the **filling procedure** (§9) — the exact commands.

**The test-arithmetic / run-result distinction (stated explicitly, as the
subtask requires):** the self-tests assert hand-derived values over
constructed inputs (e.g. the published FNV-1a 64 vectors; 8.25 − 8.0 =
0.25 ≤ 0.25 → reproduced within a pre-registered 0.25 tolerance; 8.3 −
8.0 ≈ 0.3 > 0.25 → not reproduced). That is **test-grade arithmetic
pinning the schema and the comparison rules** — it is NOT a rerun
outcome, NOT a seed result, and NOT evidence about any case. No measured
or executed cell is filled by it.

**No artifact is added to `benchmarks/` by this subtask.** The P8.20
ledger steps dictate a standard + store + rerun records, not a new
benchmark artifact; the per-run records are produced at RUN time and
stored per §4's store procedure, not in the frozen benchmark directory.
Adding one would grow the frozen directory outside any ledger step — the
P8.19 anti-silent-growth discipline is respected by NOT adding. (The
recorded derivation string and solver pins are IMPLEMENTED from the v1
artifact, never edited in it.)

## 2. What MUST be recorded for every run (ledger step 1)

Every reported result carries a complete reproducibility record — a
`flexcore-research/reproducibility-record.v1` object with these fields
(enforced by `record_schema.validate_record`; a filled field with a
template token is refused, never absorbed):

| Field | Content | Source of the requirement |
|---|---|---|
| `record_schema_version` | exactly `flexcore-research/reproducibility-record.v1` — a different string is refused (`SCHEMA_VERSION_UNKNOWN`); a schema change is a new schema version, never a silent edit | the P8.19 anti-silent-growth rule mirrored onto the record format |
| `record_id` / `run_id` | stable identifiers of the record and the run | §19.2(5) evidence linkage |
| `module_id` | one of the closed vocabulary: the research modules (`admm-dmpc`, `gnn-warm-start`, `gnn-screening`, `marl-policy`, `dynamic-controller`) and the frozen P8.06 baseline ids (`baseline/highs-tier1-lp`, `baseline/clarabel-tier2-conic`, `baseline/rule-based-priority-list`) VERBATIM | P8.06 §3 registry |
| `module_version` | the module's frozen version stamp (e.g. `flexcore-research/admm-formulation.v1` + `flexcore-research/admm-operations.v1` for ADMM; `flexcore-research/baselines.v1` for the registry) | P8.07/P8.08/P8.06 frozen-version discipline |
| `benchmark_set_version` | the cited benchmark-set version (regex `flexcore-research-benchmark-set-v< n >`; today only `flexcore-research-benchmark-set-v1` exists) — REQUIRED for every quasi-static record. **Dynamic records carry NO benchmark-set version** (the dynamic cases are not in the set — not-tested NT-10; the X-5 boundary keeps the tracks separate): they cite `inputs.dynamic_case_ref` instead | P8.19 `evaluation_citations` rule; X-5 |
| `code_version` | the code version the run executed | §19.2(5); the P3.27 `--code-version` pin |
| `component_versions` | every dependency the result's class depends on, per the per-module required-key table (HiGHS `1.15.1` for the Tier-1 baseline; Clarabel `0.11.1` for Tier 2; the torch/PyG pair for the GNN modules; the RL framework for MARL; ANDES for the dynamic module) — the value `UNPINNED-REVERIFY` is honest for DECISION-POINT keys, absence is not | D4; P8.03; the frozen artifact's solver pins; the P8.13 §5 and P8.10 §6 DECISION-POINTs |
| `determinism.seed_supported` | bool; when true, the `seeds` block and the **verbatim derivation string** `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))` (a different string is `SEED_DERIVATION_MISMATCH` — it means a new benchmark-set version); when false (ADMM — no randomness; Clarabel — the algorithm has no seed input; the rule-based baseline — no randomness), a seeds block is `SEED_POLICY_CONFLICT` — seed honesty, P7.42 | P8.04 `DeterminismMetadata`; the frozen artifact's `seeds` blocks; P8.06 §3 seed-policy column |
| `determinism.nondeterminism_sources` | the disclosed nondeterminism sources (GPU nondeterminism, parallel data-loading order, framework-version sensitivity — P8.38's list). REQUIRED NON-EMPTY for every `within_tolerance`/`distribution` record — "GPU nondeterminism unrecorded and presented as exact reproduction" is the ledger's named risk | P8.38 |
| `config.config_digest` | digest of the run configuration, format `sha256:<64 hex>` or `fnv1a64:<16 hex>` (the algorithm travels with the digest — the repo's test-grade FNV pattern is explicitly not a security primitive) | §19.2(5) "configs" |
| `inputs.input_snapshot_ids` | the input snapshot IDs (the §9.2/P8.04 provenance base carries the input snapshot id) | P8.04 |
| `inputs.input_artifact_digests` | path → digest of every input artifact — **"seeds recorded but the data pipeline not (same seed, different data)" is the ledger's named risk; the digests are the recorded data pipeline** | the ledger's risk list |
| `inputs.dynamic_case_ref` | the model/case reference for dynamic records only (every dynamic number carries its model/case reference, P8.17) | P8.17 |
| `artifacts.artifact_refs` | references into the artifact store, format `flexcore-artifact://<key>` — a local path is `ARTIFACT_REF_INVALID` ("artifacts stored only on the researcher's machine" is the ledger's named risk) | ledger step 2; §6.5 |
| `outputs.output_artifact_refs` / `outputs.output_digests` | the run outputs' store references and digests | ledger step 2 |
| `environment` | os, arch, cpu_model, logical_cores, memory, container_flag — anything the process cannot observe is recorded `unavailable: <reason>`, never guessed (the P3.27 hardware/container rule, verbatim discipline) | `benchmarks/README.md` |
| `tolerance_class` / `tolerance_ref` | `exact`, `within_tolerance`, or `distribution`; a non-exact record MUST reference the P8.38 pre-registration that declared its tolerance BEFORE the rerun (`TOLERANCE_REF_REQUIRED` otherwise) — the VALUE never lives in the record | X-8; P8.38 |
| `run_status` / `failure_reason` | `COMPLETED` or `FAILED`; a FAILED record names the failure (recorded AS A FAILURE, never as a blank or a number) | the P8.09/P8.15/P8.17 refusal discipline |

Per-module required `component_versions` keys (from
`record_schema.required_component_version_keys`; additional keys allowed
and recommended — e.g. `pandapower` when a run exercised the
grid-feasibility worker, `helics` for federated runs):

| Module | Required keys | Basis |
|---|---|---|
| `baseline/highs-tier1-lp` | `highs` | the frozen pin 1.15.1 (P8.06 §3) |
| `baseline/clarabel-tier2-conic` | `clarabel` | 0.11.1 (P8.06 §3) |
| `baseline/rule-based-priority-list` | (none) | no solver — in-crate heuristic; the registry version travels in `module_version` |
| `admm-dmpc` | (none) | the provider is in-crate; no external solver (P8.07) |
| `gnn-warm-start` / `gnn-screening` | `torch`, `pyg` | the P8.10 §6 pair — DECISION-POINT OPEN, so **no GNN record can exist until it resolves** |
| `marl-policy` | `rl_framework` | the P8.13 §5 DECISION-POINT — same honesty |
| `dynamic-controller` | `andes` | the P8.16 pin procedure |

**Units, fixed for every recorded run (report §10.2, canonical):** power
in MW/MVAr/MVA, energy in MWh; `P > 0` means export at the point of
connection; reactive-power sign is declared per grid model and device
profile, never assumed; SOC is a fraction in [0, 1] at the API boundary;
voltage kV at the nominal base, per-unit in solver representation. A
record whose outputs use any other unit convention is not enterable.

## 3. The record FORMAT — a versioned artifact schema

The schema is **`flexcore-research/reproducibility-record.v1`**, normative
as implemented in `record_schema.py` (which the reviewer reads as the
schema definition). Rules:

1. **A schema change is a NEW schema version** (`…record.v2`), frozen as
   a new validator state — never a silent edit (the P8.19
   anti-silent-growth rule and the P8.06 frozen-version discipline,
   mirrored).
2. **Refusals, not defaults**: every violation returns a named finding
   (`FIELD_MISSING`, `PLACEHOLDER_PRESENT`, `MODULE_UNKNOWN`,
   `BENCHMARK_SET_VERSION_INVALID`, `SEED_DERIVATION_MISMATCH`,
   `SEED_POLICY_CONFLICT`, `NONDETERMINISM_SOURCES_REQUIRED`,
   `TOLERANCE_REF_REQUIRED`, `DYNAMIC_RECORD_BENCHMARK_SET_FORBIDDEN`,
   `DIGEST_FORMAT_INVALID`, `ARTIFACT_REF_INVALID`,
   `FAILURE_REASON_REQUIRED`, …). Nothing is coerced, defaulted, or
   silently absorbed.
3. **A filled record with a template token** (`RECORD-AT-FIRST-RUN`,
   `TBD`, `EMPTY`, …) is refused (`PLACEHOLDER_PRESENT`) — the "not run
   hidden as a blank" failure mode, named at the record layer.
4. The record is one PART of the reproducibility standard; the schema
   version travels in the record, and the P8.23 comparison and the P8.26
   report may cite only results whose records are schema-valid.

## 4. The artifact store (ledger step 2) — the SPECIFICATION, honestly unimplemented

The ledger step says: "Store run artifacts (configs, seeds, model files,
outputs) in object storage (§6.5) with artifact references in the
results." The honest status: **no object storage exists in this
repository, and none was implemented in this loop** (infrastructure work
is outside the loop bans' allowance and outside this subtask's file
ownership). What is recorded NOW, and enforced by the tooling, is the
SPECIFICATION every run record must satisfy:

| Element | Specification |
|---|---|
| Reference format | `flexcore-artifact://<key>` — non-empty key, no whitespace; a bare local path is refused (`ARTIFACT_REF_INVALID`) |
| Stored content | the run's configs, seeds, model files, and outputs (ledger step 2) — everything the rerun needs "from the record alone" |
| Retrieval rule | a result whose artifacts are not retrievable THROUGH THE STORE REFERENCES cannot be rerun-tested and therefore cannot enter the benchmark report's claims — "stored only on the researcher's machine" is a named exclusion reason, not an inconvenience |
| Digest binding | every stored artifact's digest travels in the record (`sha256:` or `fnv1a64:`); the digest algorithm travels with the digest |
| Store implementation | an infrastructure task for the owners of §6.5 — recorded as a contract gap (§10), never invented here |

## 5. The rerun test DEFINITION (ledger step 3)

**The test:** a second engineer (not the result's author) re-runs a
selected result from its reproducibility record ALONE, on a clean
environment, and confirms reproduction within the tolerance recorded for
that module **before** the rerun.

The definition, part by part:

1. **From the record alone.** The rerunner uses only the record's fields
   and the artifact-store references — asking the author for a missing
   input is a failure of the RECORD, recorded as such (the ledger's
   risk: artifacts that exist only on the researcher's machine).
2. **On a clean environment.** The rerunner's dependency versions must
   EQUAL the record's `component_versions`; any difference is a named
   finding (`ENVIRONMENT_VERSION_DIFFERS`), never absorbed — the
   "framework version sensitivity" nondeterminism source is disclosed
   precisely so a version difference cannot hide inside a "reproduction".
3. **Identity before comparison.** The rerun record must match the
   primary on the identity fields (schema version, module id and
   version, code version, config digest, seeds, input snapshot ids,
   input artifact digests, benchmark-set version); any mismatch means
   the rerun is NOT of the recorded result → outcome
   `INVALID_RERUN_IDENTITY`, a procedural failure — no reproduction
   verdict is possible, and the finding is fixed by fixing the rerun
   procedure, never by editing the finding.
4. **Within the PRE-REGISTERED tolerance (X-8).** The tolerance is
   injected from the P8.38 per-module pre-registration, declared BEFORE
   the rerun, never fitted after the fact; a comparison without one is
   refused (`TOLERANCE_NOT_PRE_REGISTERED`, the P8.09/P8.15 discipline).
   The comparator NEVER derives, defaults, or suggests a tolerance value.
5. **Exactness is a declared class, not an assumption** (P8.38's
   distinction, enforced in `rerun_check.py`):
   - `exact` — zero deviation required. Applies to the deterministic
     modules BY THEIR RECORDED NATURE: the ADMM provider (`seed_supported
     = false`, byte-equality discipline, P8.08), HiGHS at `thread_count`
     1 with a fixed caller seed (P3.26/P8.06), Clarabel (deterministic
     algorithm, no seed input), and the rule-based baseline (no
     randomness). Declared per module at P8.38 pre-registration time.
   - `within_tolerance` — applies to ML INFERENCE reruns (GNN
     warm-start/screening inference, MARL policy inference), with the
     nondeterminism sources DISCLOSED on the record. The boundary is
     inclusive (deviation ≤ tolerance reproduces).
   - `distribution` — applies to TRAINING reruns (where training is
     claimed reproducible at all): a single-number comparison against a
     distribution is refused here with the P8.38 pointer
     (`DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38`) — comparing a
     training rerun with an inference tolerance (or vice versa) is
     exactly P8.38's named risk, and this module structurally cannot
     commit it.
6. **The outcome is recorded, and non-reproduction has teeth** (ledger
   step 4): a result that does not reproduce within its pre-registered
   tolerance carries the exclusion marking — `NOT_REPRODUCED_MARKED_FOR_
   EXCLUSION` with the statement "EXCLUDED from the benchmark report's
   claims (P8.20 ledger step 4; P8.26 must not cite this result)". The
   marking is standing, not a footnote; the P8.26 report consumes it.
7. **Ledger acceptance linkage:** "at least one result per module passed
   the rerun test" — per the five quasi-static modules plus the dynamic
   track, the per-module rerun rows of §7.2 are the evidence surface;
   the GNN held-out-topology inference rerun P8.38 requires re-proves
   the P8.12 headline metric specifically.

## 6. Dependencies and ordering (recorded, per the ledger's inputs line)

| Dependency | What this record needs from it | Its status |
|---|---|---|
| P8.04 (`04_worker_contracts.md`) | the provenance base this standard extends: `ResultProvenance` (worker/model/solver version, input snapshot id, termination status), `DeterminismMetadata`, the closed per-module payload vocabulary, the P7.42 seed-honesty discipline | implemented, awaiting human build/test |
| P8.19 (`benchmarks/research-set-v1.json`) | the frozen benchmark-set version id, the verbatim seed-derivation string, the solver pins the record schema carries | FROZEN — `flexcore-research-benchmark-set-v1` (WRITTEN-UNRUN) |
| P8.06 (`05_06_harness_baselines.md`) | the frozen baseline registry ids and seed policies (the closed `module_id` vocabulary; the exactness classes) | implemented, awaiting human build/test; the frozen RESULTS artifact does not exist until the first run |
| P8.38 | the per-module tolerance pre-registrations (the values the rerun comparator injects) and the rerun set itself | PENDING — no tolerance is pre-registered anywhere yet |
| P8.35 | the training-data manifest the dataset version links to (the GNN dataset/preprocessing versions) | NOT STARTED |
| P8.32 | the `model_artifact_ref` the record references (the signature binds to it) | PENDING (wave 7b) |
| §6.5 owners | the object-storage implementation behind the `flexcore-artifact://` references | NOT STARTED — recorded contract gap (§10.1) |

Ordering: a run's record is written AT the run (the human/lab gate); a
rerun happens only after P8.38's tolerances exist for that module; the
P8.23 comparison and P8.26 report cite only results with valid records.

## 7. Rerun-test record tables — ALL CELLS EMPTY, NO RERUN HAS HAPPENED

### 7.1 Per-run reproducibility records

One row per run, produced by the human/lab gate from the run's outputs
only. Every cell is EMPTY; no record exists.

| Run id | Module | Module version | Code version | Benchmark-set version | Seeds (derivation verbatim) | Config digest | Input artifact digests | Artifact-store refs | Tolerance class + P8.38 ref | Record valid? |
|---|---|---|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

### 7.2 Rerun-test records (ledger steps 3–4)

One row per rerun, assembled by `rerun_check.rerun_verdict` from the two
records and the injected pre-registered tolerance. Every cell is EMPTY;
**no rerun has happened**.

| Result rerun | Second engineer (not the author) | Environment matched record? | Identity findings | Pre-registered tolerance (P8.38 ref + value, declared BEFORE the rerun) | Max deviation | Outcome | Exclusion marking (if not reproduced) |
|---|---|---|---|---|---|---|---|
| EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY | EMPTY |

Filling rules (recorded now, before any rerun): cells come from
`rerun_check` outputs and the two records — never from memory, never
estimated; a refusal (`TOLERANCE_NOT_PRE_REGISTERED`,
`DISTRIBUTION_RERUN_PROCEDURE_OWNED_BY_P8_38`, `OUTPUT_KEYS_DIFFER`) is
recorded AS THE REFUSAL, never as a blank and never as a verdict; an
identity mismatch is recorded AS THE FINDING and blocks any verdict; the
outcome vocabulary is exactly `REPRODUCED_EXACT`,
`REPRODUCED_WITHIN_PRE_REGISTERED_TOLERANCE`,
`NOT_REPRODUCED_MARKED_FOR_EXCLUSION`, `INVALID_RERUN_IDENTITY`,
`BLOCKED_*`; the ledger acceptance ("at least one result per module
passed the rerun test") is met only by `REPRODUCED_*` rows.

## 8. Reviewer-verification procedure and exact filling commands (the human gate)

```sh
# (a) the in-module hand-computed self-tests (no framework, no pytest):
python apps/worker/python/reproducibility/seed_derivation.py
python apps/worker/python/reproducibility/record_schema.py
python apps/worker/python/reproducibility/rerun_check.py
#    expected: every hand-computed / theorem-derived self-test PASSes
#    (the published FNV-1a 64 vectors; mix(0)==0 and the bijection-
#    derived distinctness on 1000 states; the XOR-structure identity;
#    the schema refusals; the comparator's boundary arithmetic 8.25-8.0
#    = 0.25 <= 0.25 and 8.3-8.0 ~ 0.3 > 0.25).

# (b) the pytest mirror (written, never run in this loop):
PYTHONPATH=apps/worker/python/reproducibility python -m pytest \
  apps/worker/python/reproducibility/tests -v

# (c) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (d) the derivation-string alignment check (by-citation seam): diff
#     RECORDED_SEED_DERIVATION (seed_derivation.py / record_schema.py)
#     against every seeds.derivation field in the frozen artifact:
python -I -c "import json;d=json.load(open('benchmarks/research-set-v1.json'));s='seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))';assert all(c.get('seeds',{}).get('derivation')==s for g in d['base_cases'] for c in g.get('cases',[]) if isinstance(c,dict) and 'seeds' in c);print('derivation aligned')"

# (e) FIRST RUNS — the fills of §7.1 (human/lab inputs):
#     1. per run: record the §2 fields from the run's own outputs and
#        the artifact store; validate with record_schema.validate_record
#        (zero findings or the record is not enterable);
#     2. the dependency gates FIRST: P8.38's per-module tolerance
#        pre-registration (BEFORE any rerun — X-8), P8.06's frozen
#        results artifact for baseline results, the P8.10 §6 pair for
#        GNN records, the P8.13 §5 framework for MARL records.

# (f) FIRST RERUNS — the fills of §7.2 (ledger step 3, the verification
#     method: a SECOND engineer, from the record alone, clean
#     environment):
#     1. select the result; re-run from the record alone;
#     2. identity_findings(primary, rerun) — empty, or the finding is
#        the outcome;
#     3. rerun_verdict(primary, rerun, <P8.38 pre-registered tolerance>)
#        — fill the row from the verdict only;
#     4. a NOT_REPRODUCED_MARKED_FOR_EXCLUSION row is carried to the
#        P8.26 report as an EXCLUSION, not a caveat.
```

A green (a)–(d) earns only a one-line "human build/test gate: PASSED
(date)" appended here; failures are recorded verbatim and the ledger tick
reverts to `- [ ]`. The cells of §7 stay EMPTY until the (e)/(f) runs
fill them.

## 9. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Every reported result has a complete reproducibility record | STANDARD + SCHEMA ENFORCED (§2/§3, validator refusals); no reported result exists yet — no record exists |
| At least one result per module passed the rerun test | **UNRUN** — no rerun has happened (§7.2 EMPTY); the rerun set is P8.38's, gated on its per-module tolerances (also pending) |
| Non-reproducing results are marked and excluded | THE MARKING IS IN CODE (`NOT_REPRODUCED_MARKED_FOR_EXCLUSION` with the standing exclusion statement, §5 item 6); no result exists to mark |
| Tolerances declared per module BEFORE the rerun | OWNED BY P8.38, PENDING — the comparator structurally refuses without the injected pre-registration (`TOLERANCE_NOT_PRE_REGISTERED`) and cannot derive one (X-8) |

## 10. Contract gaps → follow-ups (recorded, not invented)

1. **No object storage exists** (§6.5): the store behind the
   `flexcore-artifact://` references is a recorded SPECIFICATION, not an
   implementation. Its implementation belongs to the §6.5 owners — never
   invented here. Until it exists, a result cannot satisfy the store
   rule and therefore cannot be rerun-tested — recorded honestly, not
   papered over with local paths.
2. **No per-module tolerance is pre-registered anywhere** (P8.38
   pending): the comparator refuses every comparison until the injected
   pre-registrations exist. The tolerance VALUES are never recorded in
   this standard — that would be fitting them before the model, service,
   benchmark, and operational risk are defined (X-8; P8.02 §5).
3. **The torch/PyG pair and the RL framework are DECISION-POINTs OPEN**
   (P8.10 §6; P8.13 §5; D4): GNN and MARL records are structurally
   unfilled until they resolve (the required `component_versions` keys
   cannot carry honest values). Recorded as dependencies, not fixed by
   inventing pins.
4. **The seed-derivation interpretation is recorded, not dictated by the
   artifact**: "splitmix64(x)" is fixed as one output of the stream
   seeded with x (`seed_derivation.py`, HONESTY HEADER item 3). Any
   different reading is a NEW derivation version requiring a NEW
   benchmark-set version (v2) — never a silent reinterpretation. The
   by-citation alignment check (§8 step d) is the seam.
5. **The record schema has no canonical serialization pinned** (the
   frozen benchmark artifact pins field ORDER; the record schema pins
   PRESENCE and vocabulary but not a byte-order). For the identity check
   the comparator canonicalizes (JSON, sort_keys), so comparison is
   order-insensitive; a byte-exact record format is a v2 schema
   candidate if reviewers need textual diffs of records. Recorded, not
   invented.
6. **Environment capture is partial on some platforms** (the P3.27
   hardware block: `/proc/cpuinfo`/`/proc/meminfo` are Linux paths): the
   schema requires the fields and accepts `unavailable: <reason>` —
   never omission, never guessing. A richer capture mechanism is the
   P3.27 harness owners' follow-up, if needed.

## 11. Non-claims (restated)

No rerun has happened; no reproducibility record exists; no rerun-test
outcome exists anywhere in this record or its code — the self-tests'
arithmetic pins the schema and the comparison rules and is not evidence
about any case. No tolerance value is recorded or suggested anywhere in
this standard (X-8 — P8.38 owns the pre-registrations). No claim that
any result is "reproducible" exists — reproduction is proven only by the
recorded rerun outcomes, of which there are none. Nothing here qualifies
any feature for field use (§2.1); the entire standard governs
simulation-only research artifacts (AGENTS.md rule 5). The record schema
is a reproducibility bookkeeping format — it stores no dispatch-shaped
concept and grants no authority (ADR-010).
