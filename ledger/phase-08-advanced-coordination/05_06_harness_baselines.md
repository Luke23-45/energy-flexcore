# P8.05 + P8.06 — The independent validation harness and the strong deterministic baselines

Date: 2026-11-13. Subtasks: P8.05 and P8.06 (`ledger_phase_8.md`
§P8.05/§P8.06). Owner roles: power-systems reviewer, not the module
author (P8.05); research lead + power-systems reviewer (P8.06). Report
trace: §13.1–§13.3; §19.2(5)(9); §12.5; §22.3; §23 Stage 8 (exit gate);
§19.3–§19.4; §12.3. Inputs: the P8.04 contracts
(`04_worker_contracts.md`); the Phase 4 feasibility gate
(`crates/flexcore-application/src/command_gates.rs`, P4.12, §12.5); the
§19.3 benchmark cases; the P8.01 baseline benchmark results
(`01_readiness_check.md`); the P8.02 research plan.

## 1. Status of this record — WRITTEN-UNCOMPILED

The crate `crates/flexcore-research` (lib.rs, validation.rs,
baselines.rs, plus the `#[cfg(test)]` suites added by this continuation)
is **WRITTEN-UNCOMPILED**: no `cargo build` / `check` / `test` / `clippy`
/ `fmt` command was executed in this loop (loop bans; ENTRY_NOTE.md).
Every type reference was written against source that was read, and the
known-answer test values are either hand-computed from the written rule
sets, published FNV-1a 64 test vectors, or literals copied from the
module's own constants — nothing is fabricated. The human build/test
gate (AGENTS.md rule 7) owns the compile/test evidence. NOTHING here is
evidence that the crate compiles or that any test passes.

## 2. P8.05 — the independent validation harness (the validator that does not trust the researcher)

Implemented in `crates/flexcore-research/src/validation.rs`
(`HARNESS_VERSION = "flexcore-research/validation.v1"`).

**Separate ownership (ledger step 3).** The harness lives in its own
composition crate, outside every research worker. The research workers
are Python (`python/workers/`, ADR-004); a Python worker cannot import a
Rust crate, so the ADR-004 process boundary IS the no-self-validation
guarantee — the harness is not importable by the code it validates. The
review-by-someone-else requirement is an ownership record, not code: the
owner of record for this module is the power-systems reviewer role, not
the research-module authors.

**Fixed stage order (cheap refusals first, the gate LAST):**

| Stage | Check | Refusal class (named, one per failure) |
|---|---|---|
| 1 `envelope_shape` | proposal id / producer identity / sequence ≥ 1 / window non-inverted / payload ≤ 1 MiB | `PROPOSAL_ENVELOPE_INVALID` |
| 2 `result_vocabulary` | closed §13.4 kind vocabulary (round-trip pinned) | `RESULT_VOCABULARY_UNKNOWN` |
| 3 `provenance_completeness` | §9.2 base + P8.32 `model_artifact_ref` + P7.42 seed honesty + the proto's bare-"ok" rule | `PROVENANCE_INCOMPLETE` / `SEED_POLICY_INCONSISTENT` / `TERMINATION_STATUS_INSUFFICIENT` |
| 4 `validity_window` | `[valid_from, valid_until)` against the INJECTED clock | `PROPOSAL_NOT_YET_VALID` / `PROPOSAL_EXPIRED` |
| 5 `snapshot_freshness` | Current / Superseded / Unknown via the injected source | `SNAPSHOT_SUPERSEDED` / `SNAPSHOT_UNKNOWN` |
| 6 `physical_revalidation` | the INJECTED platform feasibility gate — LAST | `PHYSICAL_REVALIDATION_LIMIT_VIOLATED` / `PHYSICAL_REVALIDATION_FAIL_CLOSED` / `PHYSICAL_REVALIDATION_SEAM_FAILED` |

A proposal refused by any earlier stage NEVER consumes a feasibility
attempt (pinned by the gate-spy tests: `cheap_refusals_never_consume_a_
gate_attempt`).

**The injected ports (the harness never re-implements physics).**
`IndependentFeasibilityGate::revalidate` is the port the composition
root implements over the platform's EXISTING §12.5 feasibility
pipeline. Its verdict type MIRRORS
`crates/flexcore-application/src/command_gates.rs::FeasibilityGateVerdict`
(P4.12) by citation — `Passed{attempt_id, model_id, model_content_hash,
margins, coverage}` / `LimitViolated{constraint named}` /
`FailClosed{reason named}` — with the model hash traveling on the pass
and binding the evidence (`evidence_reference` in the recorded
`feasibility:attempt/<id>:model/<mid>:hash/<hash>` format). The mirror
is the recorded alignment seam (`FEASIBILITY_VERDICT_ALIGNMENT_SEAM`);
this crate does NOT depend on flexcore-application (§4 below). AC power
flow evaluates a state; it is not AC-OPF (§12.5) — that discipline lives
in the gate, not here. `SnapshotFreshnessSource` and `HarnessClock` are
likewise injected; `SystemTime::now()` appears nowhere in the module.

**The sealed-construction fail-closed pattern.** `ValidatedProposal`
carries a PRIVATE seal field: no struct-literal construction outside the
crate, no `Default`, and `serde::Deserialize` is deliberately NOT
derived (while `Serialize` is) — a validated proposal cannot be
smuggled in from JSON. The ONLY public path that returns one is
`ValidationHarness::validate`, which refuses on ANY failed stage and
never forwards the refused proposal. Every seam error (`Err` from any
injected port) is a REFUSAL, never a pass.

**The researcher supplies no verdict.** `ResearchProposal` and
`ResearchProvenance` have NO field a research module could use to assert
its own feasibility, optimality, or safety; `diagnostics` is
human-facing prose the harness never interprets (pinned by
`the_researcher_supplies_no_verdict_diagnostics_are_not_gate_input`).

**Harness version on every result (ledger step 4).** Every
`ValidationReport` and every `ValidatedProposal` stamps
`HARNESS_VERSION`; a harness change is a NEW version (the P8.06
frozen-version discipline applied to the harness itself).

**Deliberately-wrong cases in the test suite (ledger step 2).** The
in-module suite pins: a constraint-violating proposal (gate returns
`LimitViolated` → named refusal), a fail-closed gate (validator/
infrastructure failure → refusal, never a pass), a gate seam error
(fail-closed), a provenance lie (seed claimed but missing), a bare-"ok"
termination, an expired proposal (never forwarded), a superseded/unknown
snapshot, and a non-converged-ADMM-shaped failure collapses into the
same `FailClosed` refusal class. WRITTEN-UNRUN.

**Recorded seams (not implemented here, by design).** P8.32: model-
artifact SIGNING composes into stage 3 when it lands — the
`model_artifact_ref` field is the field the signature binds to. P8.34:
wire-contract windows and gate-at-time-of-use extend stages 4–5 without
changing the harness shape.

## 3. P8.06 — the strong deterministic baselines (the comparison every module must beat)

Implemented in `crates/flexcore-research/src/baselines.rs`
(`BASELINE_REGISTRY_VERSION = "flexcore-research/baselines.v1"`).

**The three tiers (ledger step 1; solver-class discipline VERBATIM,
AGENTS.md rule 4 / §7.4):**

| Baseline | Class label (verbatim in code) | Frozen solver version | Seed policy |
|---|---|---|---|
| `baseline/highs-tier1-lp` | HiGHS — LP/QP/MIP (Tier 1); **not SOCP** | 1.15.1 (the recorded highspy pin, RESEARCH.md §2, checked 2026-11-07) | Fixed caller seed via `DeterminismMetadata` (P7.42); thread_count pinned 1 (P3.26) |
| `baseline/clarabel-tier2-conic` | Clarabel.rs — convex conic (Tier 2); **not MISOCP; no integer variables** | 0.11.1 (root `Cargo.toml`: `clarabel = "=0.11.1"`) | None (the algorithm has no seed input — recorded honestly) |
| `baseline/rule-based-priority-list` | rule-based deterministic dispatch — no solver; no optimality claim | n/a — in-crate heuristic (`priority-list.v1`) | None (no randomness) |

The solver tiers are consumed through the EXISTING
`flexcore-optimization-api` `Optimizer` port, mirrored by citation as
`BaselineOptimizer` (`OPTIMIZER_PORT_ALIGNMENT_SEAM`) — the composition
root implements the port over the real providers; the recorded versions
are facts about the workspace pins, not a claim that any build ran.

**The rule-based baseline is FULLY specified** (the frozen
`priority-list.v1` rules, written into the module docs and pinned by
hand-computed known-answer tests): validate fail-closed (named errors;
negative demand, non-finite values, SOC outside [0,1], p_min > p_max all
refused, never clipped); order assets by (priority ASC, asset_id ASC);
per interval walk the order with the candidate
`min(remaining_demand, p_max, energy_cap)` where the energy cap is
`remaining_energy_mwh / interval_hours`; skip rather than overshoot a
declared `p_min`; commit at exactly the candidate; record the deficit
honestly (under-supply is never silently filled). The known-answer tests
pin the arithmetic (6.0+4.0 split, priority/lexicographic ordering,
energy depletion across 0.5 h intervals, skip-below-p_min, the explicit
0.0 commit of an exhausted p_min-0 battery, honest deficits) and the
digest function against published FNV-1a 64 vectors.

**Frozen-version discipline (ledger step 3).** A baseline change is a
NEW baseline version, never a silent edit (`bump_version` is the only
legal change path; unparseable versions get a `.v2` suffix, never
reused). Freezing computes an FNV-1a 64 digest over the record's
canonical description (the repo's test-grade checksum pattern —
explicitly NOT a security primitive); a baseline without written
assumptions cannot be frozen (`AssumptionsMissing`); a re-run digest
mismatch is a NAMED `DigestMismatch`, never absorbed. Every record
carries its assumptions/limits (ledger step 4) so a "win" against a
weak baseline is visible — the rule-based record states plainly that
"the strength of this baseline is exactly the written rule set".

**Benchmark entry point (ledger step 2).** Every record cites the
EXISTING infrastructure: `apps/sim-runner` (the P5.18/P5.19
benchmark/replay path) over `scenarios/library/` (lib-01…lib-08, §19.3
base set). The research-case extensions (stressed scenarios, topology
changes, communication loss, asset dropout, reverse power flow) are
P8.19's versioned artifact — the registry cites the library version and
does not own a case list.

**Per-module mapping (ledger step 1):** ADMM/DMPC vs all three tiers;
GNN warm-start vs the cold-start Tier 1 solver + the rule-based record;
MARL vs deterministic optimization AND the rule-based baseline (§13.3).

**THE RESULT CELLS ARE EMPTY — deliberately.** Ledger step 2 (run every
baseline on the §19.3 cases with pinned versions/seeds/tolerances and
record objective value, feasible interval count, runtime, violations)
has NOT been executed: no benchmark run happened in this loop, and
ENTRY_NOTE boundary 4 forbids fabricating a figure. The registry as
shipped freezes the DEFINITIONS; the frozen RESULTS artifact (digest
over run outputs via `results_digest_for` / `with_results`) is produced
by the first human/lab run and only then compared against. The
P8.01-recorded baseline benchmark numbers remain the comparison
baseline until then.

## 4. Composition-crate placement rationale (the recorded decision)

`flexcore-research` is a COMPOSITION crate, like flexcore-kernel and
flexcore-plugin-host:

- It composes the platform's EXISTING seams — the §12.5 feasibility-gate
  surface (injected port), the §12.6 result vocabulary, the §13.4
  proposal vocabulary — rather than defining a new core contract.
- The dependency rule is preserved by construction: adapters depend on
  ports; the harness DEFINES ports (`IndependentFeasibilityGate`,
  `SnapshotFreshnessSource`, `HarnessClock`, `BaselineOptimizer`) that
  the composition root implements over the real gate and providers.
- Nothing depends ON it except future composition roots; the core-crate
  edge policy does not apply to it, and no core crate may depend on it
  (enforced by `tools/check_dependency_rule.py` classification, §5).
- Dependency policy honored: serde/serde_json only (both existing
  workspace deps). No solver crate, no transport, no async runtime —
  the providers are consumed by citation + mirrored port, keeping the
  crate dependency-minimal.

## 5. CORRECTION — the Cargo.toml handover note's registration claim was false at kill time

The prior agent's note in `crates/flexcore-research/Cargo.toml` claimed
the root `Cargo.toml` member entry and the
`tools/check_dependency_rule.py` registration "were edited ADDITIVELY in
the same wave". **That claim was FALSE when written**: the prior agent
was killed before performing either edit, and neither the root members
list nor the checker contained flexcore-research. The continuation agent
(2026-11-13) completed the registration:

- root `Cargo.toml`: member `"crates/flexcore-research"` added
  additively (the 20th workspace member) with the recorded
  composition-crate comment, mirroring the kernel/plugin-host entries;
- `tools/check_dependency_rule.py`: `"flexcore-research"` added to
  `COMPOSITION_CRATES` and `"crates/flexcore-research"` to
  `MEMBER_DIRS`, mirroring exactly how flexcore-kernel and
  flexcore-plugin-host are registered;
- the false note in the crate's own `Cargo.toml` replaced with this
  correction.

Actual output of `python -I tools/check_dependency_rule.py` run AFTER
those edits (exit code 0):

```text
OK — dependency rule holds for 20 workspace members.
```

(The console rendered the em dash with mojibake in the captured output;
the line is the checker's fixed `OK — dependency rule holds for {n}
workspace members.` string with n = 20.)

## 6. Verification method and status

- Import-graph / dependency separation: `python -I
  tools/check_dependency_rule.py` → `OK — dependency rule holds for 20
  workspace members.` (quoted above, exit 0). The research workers'
  inability to import the harness is structural (ADR-004 process
  boundary; §2).
- "Run the deliberately-wrong suite": WRITTEN-UNRUN — no cargo test was
  executed (loop bans). The suite names and pins every required case;
  the human build/test gate runs it.
- "Re-run one baseline case from the frozen artifact's inputs": NOT
  POSSIBLE YET — the results artifact does not exist until the first
  real benchmark run (§3, empty result cells). The reproduction check
  (`confirm_reproduction`) and its named-mismatch error are the code
  path that will verify it.
- Honest status: **WRITTEN-UNCOMPILED**, per §1.
