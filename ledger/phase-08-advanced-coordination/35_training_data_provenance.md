# P8.35 — Training-data provenance, license, and poisoning controls (STANDARD — every per-instance cell EMPTY, no dataset exists)

Date: 2026-11-14. Subtask: P8.35 (`ledger_phase_8.md` §P8.35). Owner role:
Research lead. Report trace: §9.3 (supply chain, by analogy); §19.3–§19.4;
§13.1 (training-data scope). Inputs: P8.03 (`03_research_license_audit.md`
— the code-dependency audit this record extends to DATA; the ANDES GPLv3
isolation design §3; the torch/PyG DECISION-POINT §4); P8.10
(`10_gnn_warmstart.md` §4–§5 — the held-out-set RECOMMENDATION this
manifest confirms, and the leakage controls enforced at ingestion);
P8.13 (`13_marl_design.md` §6 — P8.35 owns the MARL training-data
provenance, the worker carries only the `training_data_ref`); P8.20
(`20_reproducibility.md` — the reproducibility record standard and the
`flexcore-artifact://` store specification the dataset versions link
into); P8.19 (`benchmarks/research-set-v1.json` — the verbatim
seed-derivation string); `scenarios/library/INDEX.md` + the per-case
`grid_model.fixture` fields (the topology-family facts recorded in §4);
`gnn_worker/gnn_evaluation.py::check_heldout_overlap` /
`verify_leakage_controls` (the evaluation-time audit code); ENTRY_NOTE
boundaries 4–5.

## 1. Status of this record — a STANDARD + a MANIFEST; no training dataset exists

This record is the **training-data provenance standard** (ledger steps
1–4), and — with the versioned manifest artifact it pins — the artifact
the P8.10 held-out RECOMMENDATION was waiting for. **No training dataset
exists anywhere in this repository**: nothing was generated, no
simulator-produced instance was created, no per-instance seed was
drawn, no digest was computed, and no ingestion/quarantine run happened
(all loop bans; ENTRY_NOTE boundary 4). Every per-instance and
per-dataset measured cell below is EMPTY and is filled only by the
human/lab gate at dataset-generation time, via §9's procedure.

What exists and is verifiable to the extent the loop allows:

- the **manifest artifact** `apps/worker/python/training-data-manifest-v1.json`
  (schema `flexcore-research/training-data-manifest.v1`): the CONFIRMED
  topology-family assignment (train vs held-out vs regime hold-out),
  the named structurally-unseen hold-out families, the instance-ID
  scheme the GNN worker's `trained_topology_ids` consumes, the
  leakage-control requirements, the license register, and EMPTY
  per-instance rows — WRITTEN, never executed against a generator;
- the **record standard** (§2): the fields every training dataset must
  carry, mirrored onto the P8.20 reproducibility discipline (digests,
  artifact-store references, no silent growth);
- the **poisoning controls** (§6): the ingestion validation and
  quarantine procedure as a RECORDED procedure — the poisoning-
  detection run is a HUMAN GATE and is UNRUN.

**The confirmation this record performs (the P8.10 seam).** P8.10 §4
fixed only the RULE and named its concrete hold-out assignment a
RECOMMENDATION, "UNCONFIRMED until the P8.35 manifest exists". §4 below
confirms that assignment **with two recorded findings** (the fixture-
family facts of the §19.3 library) and resolves them at the record
level — this un-blocks the P8.12/P8.21 scope-flag blocker AT THE RECORD
LEVEL only: no training run is possible until the framework
DECISION-POINTs resolve, the CBF derivation is signed (P8.13 §3), and
the human gate runs the generation procedure. The reviewer's overlap
re-check (P8.10 verification method) remains the evidence, not this
record's claim.

## 2. The per-dataset provenance record standard (ledger step 1)

Every training dataset — existing or future — carries ONE record with
these fields, stored with the dataset and referenced by every model
artifact trained on it (via `training_data_ref`). A field that cannot
be filled is recorded `unavailable: <reason>` at generation time, never
omitted and never guessed (the P3.27 discipline, mirrored). The record
is machine-checkable: it feeds `verify_leakage_controls` (P8.12's
evaluation-time audit) and the P8.20 reproducibility record's
`inputs.input_artifact_digests`.

| Field | Content | Basis |
|---|---|---|
| `dataset_id` / `dataset_version` | Stable ID + monotonically versioned string (`<id>/v<n>`); a content change is a NEW version, never a silent edit (the P8.19 anti-silent-growth rule, mirrored onto datasets) | P8.35 step 4; P8.10 §5 versioning row |
| `module_ids` | The research modules the dataset trains (`gnn-warm-start`, `gnn-screening`, `marl-policy`, `dynamic-controller`) — the closed P8.20 module vocabulary | P8.20 §2 |
| `source` | Simulator-generated cases only (0.x rule): `apps/sim-runner` generation over the declared family parameter sets, with the **generator version** (the sim-runner/simulator commit or release) recorded | P8.10 §5 source row; §13.1 |
| `generation_method` | Per instance: generator version + the instance's parameter-set reference + the instance seed (derivation verbatim: `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))`, master seed recorded per dataset) | P8.10 §5; the frozen artifact's derivation string; P7.42/P8.20 seed honesty |
| `topology_family` | The manifest family ID (`fam-train-radial-small`, `fam-holdout-radial-large`, …) the instance was generated from | the manifest (§4) |
| `size` | Instance count + node/asset-count bounds actually generated (a filled cell at generation time; the manifest deliberately records NO number — none exists) | P8.10 §5 size row |
| `license` + `license_consequence` | The license register entry (§5) and the recorded distribution/use consequence with its disposition | ledger step 2 |
| `content_digest` | The dataset's digest at freeze time, algorithm stated (`sha256:<64 hex>` or `fnv1a64:<16 hex>`), pinned BEFORE any training run | ledger step 4; the "same seed, different data" named risk (P8.20 §2) |
| `transformation_history` | The ordered list of transforms applied to the raw generated records (normalization-statistic computation, feature extraction, labeling) — each with its own code version and, where it touches statistics, the SPLIT it was computed on (training-only) | ledger step 1; P8.10 leakage control 2 |
| `per_sample_lineage` | Per instance (where feasible — required for all simulator-generated datasets, which 0.x training data exclusively is): instance ID → family, parameter-set ref, seed, generation timestamp, source-record digest | ledger step 1 "per-sample lineage where feasible" |
| `leakage_controls` | The control declarations `verify_leakage_controls` checks: `normalization_split == "training_only"`, `regime_seeds_disjoint_from_holdout: true`, `heldout_topology_overlap_checked: true`, plus the recorded `check_heldout_overlap` result (empty overlap) | P8.10 §5; `gnn_evaluation.py` |
| `artifact_ref` | The `flexcore-artifact://` store reference the frozen dataset lives at (the P8.20 store specification; a local path is not a store reference) | P8.20 §4 |
| `quarantine_log_ref` | The reference to the ingestion quarantine record (§6) — non-empty once ingestion has run; before that the cell says so | ledger step 3 |

## 3. The inventory (ledger step 1) — per module, every per-dataset cell EMPTY

| Module | Datasets it will train on | Source | License | Version | Digest | Status |
|---|---|---|---|---|---|---|
| `gnn-warm-start` | generated topology instances over the manifest's training families (§4) + labels from exact HiGHS solves (P8.06 Tier-1 class) | `apps/sim-runner` generation (planned; UNRUN) | synthetic — repository-generated (§5, G-1) | EMPTY | EMPTY | NOT GENERATED |
| `gnn-screening` | same instance base; labels = true active-constraint/contingency sets from the exact solves | same | same | EMPTY | EMPTY | NOT GENERATED |
| `marl-policy` | scenario-artifact episodes from the platform simulation over the declared environment model (P8.13 §2.3: by citation, no simulator import) — the environment's model version + generator version + seeds recorded per P8.35 | platform simulation outputs as scenario artifacts (planned; UNRUN) | synthetic — repository-generated (§5, G-1) | EMPTY | EMPTY | NOT GENERATED |
| `dynamic-controller` | (if the dynamic track ever trains) ANDES output traces consumed as UNTRUSTED scenario artifacts — never ANDES-derived code | ANDES 2.0.0 outputs via the isolated worker (§5, G-3) | GPLv3 isolation consequence recorded (§5, G-3) | EMPTY | EMPTY | NOT GENERATED |

No third-party, scraped, downloaded, or "borrowed" dataset is inventoried
because none exists and none may be introduced without a license-
register entry FIRST (§5, rule L-0) — the ledger's named risk ("a
dataset borrowed without a license check") is answered by ordering, not
by trust.

## 4. The held-out-topology manifest — CONFIRMED, with two recorded findings

**The artifact:** `apps/worker/python/training-data-manifest-v1.json`
(schema `flexcore-research/training-data-manifest.v1`; the versioned
manifest ID is `flexcore-research/training-data-manifest-v1`). It is
ADDITIVE; nothing in `benchmarks/` was touched (the frozen benchmark
set is P8.19's; the manifest is training-data provenance, not a
benchmark case, and adding it there would conflate the two registers).

**Fact base (read from the library records, verifiable):** the §19.3
scenario library (`scenarios/library/`, lib-01…lib-08) carries exactly
TWO grid fixtures across its eight cases — `three_bus_radial`
(lib-01, lib-02, lib-04, lib-05, lib-06, lib-07, lib-08) and
`meshed_two_feeder` (lib-03 only; `TopologyScope::RadialAndMeshed` per
P4.34). This is the FACT BASE of the manifest, not a new claim.

**Finding F-1 (regime families are not topology families).** The P8.10
RECOMMENDED hold-out assignment ("instances of the topology families of
lib-06, lib-07, lib-08") interpreted literally would hold out cases
whose grid fixture is the SAME `three_bus_radial` fixture the training
side (lib-01…lib-05) uses — the lib-06/07/08 distinction is a
REGIME/stressor distinction (PV ramp, asset dropout, reverse power
flow), not a topology distinction. Taken literally it would produce
exactly the P8.12 named risk: "held-out topologies that are
near-duplicates of training ones". **Resolution (record level):** the
manifest assigns lib-06/07/08-derived scenario material to the
HELD-OUT OPERATING-REGIME track (which P8.10's own third row already
establishes), and defines the topology hold-out as generator-produced
instance families that are structurally disjoint from every training
family by construction (§4.1).

**Finding F-2 (the meshed family vs the declared scope).** The P8.10
RECOMMENDED training set (families of lib-01…lib-05) includes the
MESHED `meshed_two_feeder` fixture (lib-03), while the P8.10 §4 D3
obligation declares balanced radial distribution feeders as the 0.x
scope RECOMMENDATION, with meshed topologies OUT of the declared scope
unless an explicitly scoped design decision adds them. **Resolution
(record level):** the manifest EXCLUDES the meshed family from GNN
training topology instances by default (the D3-recommendation-
consistent disposition) and records the alternative — including it via
an explicit `RadialAndMeshed` scope decision, mirroring the P4.34
adapter scope — as available ONLY through a new recorded decision
before training. D3 itself remains OPEN (`.agent/DECISIONS.md`); this
record closes nothing.

### 4.1 The family assignment (confirmed at manifest time)

| Manifest family ID | Structural definition (DECLARED HERE — a manifest definition, not a measurement) | Track | Lineage |
|---|---|---|---|
| `fam-train-radial-small` | balanced radial feeder; 3–12 nodes; single spine, lateral depth ≤ 2 | TRAINING | radial lineage of the library's `three_bus_radial` fixture family (lib-01/02/04/05), generator-parameterized |
| `fam-train-radial-medium` | balanced radial feeder; 13–48 nodes; spine + laterals, lateral depth ≤ 3 | TRAINING | same lineage, larger parameter range |
| `fam-holdout-radial-large` | balanced radial feeder; 49–200 nodes; lateral depth ≤ 3 | HELD-OUT (topology; size transfer per P8.21) | disjoint by node-count bound from both training families |
| `fam-holdout-radial-deep-lateral` | balanced radial feeder; any node count; lateral depth ≥ 4 | HELD-OUT (topology; structural shape) | disjoint by lateral-depth bound from every training family |
| `fam-holdout-radial-open-ring` | normally-open ring feeder (radial in operation, one normally-open tie); any node count | HELD-OUT (topology; structurally unseen shape) | a feeder shape ABSENT from the library entirely — the P8.10 "at least one topology family absent from the library" requirement, named |
| `fam-regime-holdout-lib-06-07-08` | the lib-06/07/08 scenario material (PV ramp, asset dropout, reverse power flow) over the library fixture topology | HELD-OUT (operating regime; topology FIXED) | Finding F-1's resolution; matches P8.10's held-out-regimes row |
| `fam-meshed-two-feeder` (lib-03) | the library's meshed fixture | EXCLUDED from the declared GNN scope (Finding F-2); re-includable only by an explicit scoped decision | P4.34 `RadialAndMeshed` analogue |

**Structural disjointness is by construction, not by inspection:** a
training-family instance (node count 3–48, lateral depth ≤ 3, no ring,
no mesh) cannot equal a holdout-family instance (node count 49–200, OR
lateral depth ≥ 4, OR ring, OR meshed). The reviewer's overlap check
(the ledger's verification method) verifies the manifests' lists — and
`check_heldout_overlap` re-verifies in code at evaluation time on the
instance IDs actually generated (exact string equality; no
fuzzy/prefix match passes).

**Instance-ID scheme (what the GNN worker's `trained_topology_ids`
consumes):** `topology_instance_id = "<family_id>-<4-digit sequence>"`
(e.g. `fam-train-radial-small-0001`), with each instance's row (seed,
parameter-set ref, generator version, digest) recorded in the
manifest's `per_instance_rows` at generation. **Until generation, the
trained-instance list is EMPTY — and the scope consequence is
fail-closed by design: the P8.11 worker's `evaluate_topology_scope`
would flag EVERY topology out-of-scope (outputs NULL, termination
`failed`), so no inference output can exist before a dataset does.**
That is the honest record-level state of the P8.12/P8.21 un-blocking:
the blocker is lifted at the record level (a confirmed assignment
exists to check against); no inference or evaluation result exists or
can exist until the human gates run.

## 5. The license register (ledger step 2) — datasets inherit the P8.03 discipline

Rule L-0 (ordering): **no dataset enters training before its register
row exists** — a dataset whose license forbids the intended use is
EXCLUDED, recorded as excluded, never used quietly (the ledger's
verbatim requirement).

| Register ID | Dataset class | License | Source of the claim | Consequence + disposition |
|---|---|---|---|---|
| G-1 | Repository-generated synthetic training data (sim-runner over declared family parameter sets; the only 0.x GNN/MARL training-data class) | The repository's license governs generated artifacts; the INPUTS are the synthetic fixtures (no third-party data, no IEEE model — the per-case IEEE exclusion records in the library apply) | `scenarios/library/INDEX.md`; the per-case exclusion records | No distribution restriction beyond the repository's own; disposition: usable for training, distribution subject to the repository license — recorded; final legal wording at the first real dataset is a human-gate item |
| G-2 | Labels derived from exact solver runs (HiGHS 1.15.1 / Clarabel 0.11.1 outputs as data) | The solvers are Apache/YMIT-family (recorded in P8.06 §3 pins); outputs-as-data carry no copyleft consequence under the recorded families | P8.06 §3; P8.03 §2 | Usable; the UNVERIFIED-until-pin label carries over from P8.03 |
| G-3 | ANDES-derived traces (dynamic track only, IF that track ever trains) | **GPLv3 isolation consequence**: ANDES 2.0.0 is GPLv3 (P8.03 §2); FlexCore consumes ANDES OUTPUTS as data — traces cross the worker boundary as UNTRUSTED scenario artifacts (P8.03 §3 rules 3/5/6) — never ANDES-derived code; any distribution of ANDES-derived material goes through legal BEFORE it happens (P8.03 §3 rule 4) | P8.03 §2/§3; RESEARCH.md R8-C2 | Disposition: training-data OUTPUTS inherit the same isolation design as the code boundary — traces may be consumed as untrusted data, never vendored into the repository as redistributable content without legal sign-off (gate G-4 OPEN); recorded, not decided |
| G-4 | Framework-training artifacts (torch/PyG or the RL framework) | The frameworks are permissive families (BSD-style/MIT/Apache-2.0, ALL UNVERIFIED) — but the framework choice itself is the OPEN torch/PyG DECISION-POINT (P8.03 §4; P8.10 §6; P8.13 §5) | P8.03 §2/§4 | The DECISION-POINT is CARRIED, not closed here: no framework, therefore no framework-specific dataset artifact exists; the register row is recorded so the first real dataset inherits it |

No license consequence above is closed by this record: every row
carries the P8.03 labels (UNVERIFIED until primary-source confirmation
at pin time; legal confirmation gates for G-1 wording and G-3 are the
recorded human gates). Zero-unknown-license claims are never made.

## 6. Poisoning controls (ledger step 3) — RECORDED PROCEDURE, the detection run is a HUMAN GATE, UNRUN

The ledger's requirement: malformed or poisoned records cannot SILENTLY
corrupt the experiment record. The controls, in enforcement order:

| # | Control | Mechanism | Status |
|---|---|---|---|
| P-1 | Schema validation at ingestion | Every generated record is validated against its dataset's declared schema BEFORE it enters the dataset: required fields, types, units (MW/MVAr/MWh; SOC a fraction in [0,1]; voltage pu > 0), closed vocabularies (quality states, outcome sets). A violation is a QUARANTINE with a record, never a silent drop and never a silent train-on | RECORDED PROCEDURE — the ingestion run is a human gate, UNRUN |
| P-2 | Range validation at ingestion | Physical ranges per the canonical units and per-family parameter bounds (a node count outside the family's declared bound is malformed; a negative MW magnitude is malformed). Out-of-range → quarantine with a record | RECORDED PROCEDURE — UNRUN |
| P-3 | Duplication check at ingestion | Exact-duplicate instance IDs and duplicate content digests are refused (a duplicated record double-weights an instance — a distribution change without a record). Duplicates → quarantine with a record | RECORDED PROCEDURE — UNRUN |
| P-4 | Outlier/anomaly screening | A RECORDED screening procedure (per-family parameter bounds, physical-plausibility checks, distribution-shift summary vs the family declaration) whose OUTCOME IS REVIEWED BY A HUMAN before the dataset version is frozen — the screening run is the human gate; nothing auto-removes a record (an auto-removal would itself be a silent distribution change) | RECORDED PROCEDURE — UNRUN |
| P-5 | Digest pinning BEFORE training | The dataset's `content_digest` is computed and recorded at freeze time, before any training run; the P8.20 record carries it in `inputs.input_artifact_digests` so a rerun trains on the same data ("same seed, different data" is structurally visible) | STANDARD (§2); no digest exists yet |
| P-6 | Quarantine is WITH a record | Every quarantined record's entry (instance ID, violation class, the record itself) is kept in the dataset's `quarantine_log` — the ledger's verification method ("re-run ingestion on a dataset with injected poisoned records and confirm quarantine") replays this log | PROCEDURE DEFINED; the injected-poison verification run is UNRUN |
| P-7 | Never silently dropped, never silently trained on | A quarantine is a RESULT: the dataset version's record states how many records were quarantined and why; a dataset whose quarantine log is inconsistent with its instance count is not freezable | STANDARD (§2); no dataset exists to enforce it on |

Why this is honest about status: every control above is a written
procedure over data that does not exist. The Phase 8 loop ran nothing;
the poisoning-detection verification (inject poisoned records →
confirm quarantine with a record) is explicitly a HUMAN GATE input,
listed in §9. No claim is made that any control has ever fired.

## 7. Versioning and the artifact link (ledger step 4)

- A dataset version is FROZEN (content digest + instance rows complete
  + quarantine log consistent) before any training run references it.
- The model artifact's `training_data_ref` names the
  `flexcore-artifact://` store key of the frozen dataset version; the
  P8.20 reproducibility record carries the dataset digest in
  `inputs.input_artifact_digests`; the P8.32 signing binds the model
  artifact whose manifest carries that ref. Chain: **manifest family →
  instance rows → frozen dataset version (digest) → model artifact
  (signed) → P8.20 rerun record**. A rerun that cannot resolve the
  same dataset version from the store references cannot be an identity-
  true rerun (P8.20 §5 identity fields include the input artifact
  digests).
- A dataset content change is a NEW `dataset_version`; the P8.19
  anti-silent-growth rule applies verbatim. The manifest itself is
  versioned the same way: a family-assignment change produces
  `training-data-manifest-v2`, never a silent edit of v1.

## 8. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Every dataset has source/license/version recorded | STANDARD + REGISTER + MANIFEST written (§2/§4/§5); **no dataset exists**, so the inventory's per-dataset cells are EMPTY (§3) — the standard is the deliverable this loop can honestly produce |
| Every license consequence has a disposition | DONE at the register level (§5, one disposition per row, labeled UNVERIFIED/human-gate where it is); no real dataset has ever exercised it |
| Poisoned/malformed records are quarantined with a record | PROCEDURE RECORDED (§6, P-1…P-7); the injected-poison verification run is UNRUN (human gate) |
| Dataset version is linked to the model artifact | THE LINK RULE IS RECORDED (§7); no dataset version and no model artifact exist, so no actual link exists |

## 9. Reviewer-verification procedure (the human gate)

```sh
# (a) the manifest artifact is well-formed JSON and pins what §4 records:
python -I -c "import json;m=json.load(open('apps/worker/python/training-data-manifest-v1.json'));assert m['manifest_schema']=='flexcore-research/training-data-manifest.v1';assert m['manifest_version']==1;fams={f['family_id']:f['track'] for f in m['topology_families']};assert fams['fam-train-radial-small']=='training' and fams['fam-holdout-radial-large']=='held_out_topology' and fams['fam-meshed-two-feeder']=='excluded_from_declared_scope';print('manifest structure OK')"

# (b) the fixture FACT BASE against the library records (Finding F-1/F-2):
grep -h '"fixture"' scenarios/library/lib-0*.json | sort | uniq -c
#    expected: 7x "three_bus_radial", 1x "meshed_two_feeder"

# (c) the overlap check the ledger names, once instances exist: run
#     gnn_worker.check_heldout_overlap over the manifest's generated
#     trained vs held-out instance-ID lists (exact string equality);
#     any non-empty overlap is a finding against the generalization
#     claim, never absorbed.

# (d) the FIRST DATASET (human/lab): run the generation procedure per
#     family (generator version + per-instance seeds recorded), run the
#     §6 ingestion controls, INJECT poisoned records and confirm the
#     quarantine log (the ledger's verification method), freeze the
#     version + digest, fill §3's EMPTY cells from the outputs only.

# (e) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
```

A green (a)–(c) + (e) earns only a one-line human-gate note; failures
are recorded verbatim and the ledger tick reverts to `- [ ]`.

## 10. Contract gaps → follow-ups (recorded, not invented)

1. **The meshed-family scope question (Finding F-2) needs an owner
   decision before any training instance list is frozen**: exclude
   (default) or explicitly scope `RadialAndMeshed`. Recorded as an open
   disposition, never defaulted silently.
2. **The P8.11 model manifest is request-side** (the manifest-in-request
   stopgap): the trained topology list travels per request until the
   P8.20 store + P8.32 signing exist (P8.10 §7). The dataset manifest's
   instance rows are the source the request-side list must equal; a
   mismatch is a finding. Follow-up owned by the P8.20/P8.32 owners.
3. **The ingestion validator does not exist as code** (§6 is a
   procedure, not a module): implementing it is a follow-up for the
   first dataset wave, against the schema of §2 — never invented
   inside another owner's worker files this wave.
4. **The `benchmarks/` register and this manifest are separate
   registers by design** (the frozen benchmark set is measurement
   provenance; the manifest is training-data provenance). A case that
   is both (a benchmark case whose scenario material trains a model)
   must appear in BOTH registers with cross-references — recorded as a
   rule, exercised by nothing yet.

## 11. Non-claims (restated)

No training dataset exists; no instance was generated; no seed was
drawn; no digest was computed; no ingestion or quarantine run happened;
no model artifact exists to link to. The family assignment is a
CONFIRMED RECORD-LEVEL disposition over the library's verifiable
fixture facts, subject to the reviewer's overlap re-check — it is not
evidence that any model generalizes, and X-7 (generalization claimed
only over the tested range) binds every future claim. Nothing here
grants field authority or touches the dispatch lifecycle (ADR-010;
simulation-only, AGENTS.md rule 5). The torch/PyG and RL-framework
DECISION-POINTs and D3 remain OPEN.
