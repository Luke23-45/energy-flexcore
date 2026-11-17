# Phase 15 wave 1, Agent B — close-out record (P15.07–P15.11)

| Field | Value |
|---|---|
| Wave / agent | Phase 15 wave 1, Agent B (P15.07–P15.11 — BESS degradation/warranty) |
| Status | WRITTEN-UNRUN throughout: every module and test file is written complete, never compiled, never executed. No degradation number computed, no SoH estimated, no warranty claim asserted, no pack exported, no skill/dispatch claim anywhere. The ledger was NOT edited (no checkbox ticked — outside this agent's ownership per the brief). |
| Decision records | `records/21_p15_07_bess_degradation_data_model.md`, `records/22_p15_08_degradation_cost_curve.md`, `records/23_p15_09_soh_ingestion.md`, `records/24_p15_10_warranty_object_model.md`, `records/25_p15_11_warranty_evidence_pack.md`, this close-out record. P15-E-3 registered OPEN with DEFAULT (a) (LFP + NMC first per BLAST-Lite [S-147]) in record 21; agents never close it. |

## 1. Files created / modified (complete list)

| File | Change |
|---|---|
| `crates/flexcore-domain/src/bess_degradation.rs` | NEW (P15.07): chemistry scope (P15-E-3), monotonic counters with weakest-provenance merge, DOD × mean-SOC histogram, temperature exposure, high-SOC dwell, append-ordered SoH observations, the aggregate ledger, recorded statements/seams; complete test module |
| `crates/flexcore-domain/src/bess_soh.rs` | NEW (P15.09): BMS SoH report, staleness config (7-day ASSUMPTION default), the P3.13-pattern max-age evaluation, the creditability gate, the estimation DEFERRAL statement and the NOT-IMPLEMENTED method survey; complete test module |
| `crates/flexcore-domain/src/bess_warranty.rs` | NEW (P15.10): warranty terms (retention schedule, cycle/throughput limits, declared-only RTE/availability, required citation), augmentation events, the pure headroom computation with NotCreditable enforcement and reference-structure statements; complete test module |
| `crates/flexcore-domain/src/warranty_evidence.rs` | NEW (P15.11): digest-chained evidence pack (test-grade FNV-1a 64, pinned to standard vectors), P8.32-mirrored provenance record, scheme-prefix key-reference refusal without echo, `verify_chain`, the in-pack claim-support boundary; complete test module |
| `crates/flexcore-domain/src/lib.rs` | Additive module registrations only (4 module lines with provenance comments) — landed FIRST per the wave protocol |
| `crates/flexcore-optimization-api/src/degradation_cost.rs` | NEW (P15.08): the linear/piecewise-linear cost-curve data contract, convexity gate with reshape guidance, beyond-coverage refusal, versioned + provenance-required declarations, chemistry scope; complete test module |
| `crates/flexcore-optimization-api/src/lib.rs` | Additive module registration only |
| `crates/flexcore-optimization-api/src/problem.rs` | ADDITIVE ONLY: four P15.08 solver-class guard tests appended to the existing `#[cfg(test)]` module (LP stays LP; MILP stays MILP; cones still ClassMismatch; MISOCP still refused). No IR type or validator touched |
| `crates/flexcore-application/src/scheduling_objective.rs` | ADDITIVE ONLY: `DEGRADATION_INPUT_DATA_SCHEMA_VERSION = 2` consts + statement (with the recorded reasoning that `OBJECTIVE_SCHEMA_VERSION` is NOT bumped — the term set/shape is unchanged), one `ObjectiveError` variant (`InvalidDegradationCostData`), one additive `ObjectiveInputs` field (`degradation_cost_curves`, empty default), validation + term-provenance wiring for the curves |
| `crates/flexcore-application/tests/scheduling_objective.rs` | ADDITIVE ONLY: five P15.08 wiring tests appended |
| `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/*` | The six records listed above (the directory was created at implementation time) |

NOT touched: `ledger_phase_15.md`, `plan.md`, `context.md`, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, `crates/flexcore-contracts/src/units.rs` (read-only), `crates/flexcore-domain/src/bess_model.rs` (the P3.06 placeholder superseded AS DATA, not rewritten), `crates/flexcore-application/src/envelope_staleness.rs` and `crates/flexcore-ports/src/telemetry_source_contract.rs` (patterns/integration points mirrored by citation), `crates/flexcore-research/*` (P8.32 mirrored by citation), and every file in Agent A's ownership. The never-granted permission set is byte-identical (no permission surface edited).

## 2. Permitted-tool outputs (verbatim)

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 28 workspace members.
```

(The `ù` in the third output is the console rendering of the tool's
em-dash separator under the Git Bash code page — quoted verbatim per the
honesty rules.) All three tools report no failures; no build was run and no
test was executed (conventions §1.3 — execution is the human gate; the
first human build of the workspace gates all implementation).

## 3. Deviations and decisions recorded

1. **`OBJECTIVE_SCHEMA_VERSION` NOT bumped** (record 22 §1): P3.14 requires
   a version bump for a TERM change; the `C_degradation` term shape is
   unchanged — only the accepted INPUT DATA changed, which carries its own
   `DEGRADATION_INPUT_DATA_SCHEMA_VERSION = 2`. Bumping the objective
   schema would have broken every existing v1 config construction for no
   vocabulary change.
2. **SoH staleness mirrored by citation, not by dependency** (record 23):
   `envelope_staleness.rs` lives in flexcore-application and the domain
   crate cannot depend on it; the rule shape (configured max-age, P2.08
   equality convention, stale-never-silently-used) is mirrored in
   `bess_soh.rs` and pinned by tests. No ports or application file edited.
3. **Chemistry vocabulary mirrored by citation in flexcore-optimization-api**
   (record 22): the crate's edge set is flexcore-contracts +
   serde/serde_json/time/clarabel, so `DegradationChemistryLabel` mirrors
   the domain enum with the alignment recorded in the module header.
4. **P3.06 placeholder left in place** (record 22 §3): superseded AS DATA —
   the declared curve is the optimizer input, the placeholder metadata
   remains the evaluation record; `bess_model.rs` untouched.
5. **SoH observation ingest stamp** (record 23): a P15.07 ledger `SohObservation`
   carries no separate transport receipt; when the headroom computation
   adapts it to a `BmsSohReport`, the observation time doubles as the ingest
   time — a recorded convention, documented at the adaptation site.
6. No new crate, no new dependency, no build, no test run. No checkbox
   ticked anywhere.

## 4. Handoffs

| To | What |
|---|---|
| Agent A (wave 1) | `crates/flexcore-domain/src/lib.rs` already carries Agent B's four module lines — re-read before your additive edit, as agreed; `scheduling_objective.rs` edits are additive and confined to the degradation paths (no term-vocabulary change); if you touch the file, re-read first |
| Wave 2, Agent B (P15.19/P15.20) | P15-E-3 is registered OPEN with DEFAULT (a) in record 21 §4; the E-decision record itself is yours (P15.19); the close audit should re-run the three tools and verify this close-out's file list |
| Wave 2 owners (P15.12/P15.13/P15.16) | The adapter-side wiring that feeds `BmsSohReport` (P15.09 §5) and any health-score consumption of the P15.07 ledger — scores stay RECORDS, never dispatch inputs |
| Human gates | First workspace build (all modules UNRUN); per-chemistry degradation parameters before any cost curve activates (P15-E-3 closure); production SHA-256 pack digest + signing identity (deployment requirement, P15.11); registered SoH max-age per deployment |
