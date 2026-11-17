# Phase 15 wave 1, Agent A — close-out record (P15.01–P15.06, forecasting core)

| Field | Value |
|---|---|
| Scope | P15.01–P15.06 (forecasting core) per the phase-15 plan §3 wave-1 ownership |
| Agent | Phase 15 wave 1, Agent A |
| Status | WRITTEN-UNRUN end to end: no install, no build, no test run (conventions §1); every deliverable is a design, rule set, registry, or test file whose execution is the human gate. No forecast is produced by this program; every evaluation cell is EMPTY; every model registers UNTRAINED; no skill is claimed anywhere. |
| Records | `01_p15_01_forecasting_architecture.md`, `02_p15_02_load_forecasting_baselines.md`, `03_p15_03_pv_weather_seam.md` (P15-E-1 OPEN + DEFAULT), `04_p15_04_price_ingestion_entsoe.md` (P15-E-2 OPEN + DEFAULT), `05_p15_05_forecast_evaluation.md`, `06_p15_06_model_registry.md`, this close-out |
| Ledger | NOT edited: no checkbox was ticked (the ledger is outside this agent's file ownership; per conventions §2.2 the ticks are applied by the coordinator with the count==1 heredoc discipline) |

## 1. Files created / modified

| Path | Change |
|---|---|
| `crates/flexcore-ports/src/weather_data.rs` | NEW — the P15.03 weather-data ingestion port |
| `crates/flexcore-ports/src/price_signal.rs` | NEW — the P15.04 price-signal ingestion port |
| `crates/flexcore-ports/src/lib.rs` | MODIFIED (additive) — two `pub mod` declarations, two doc-table rows, SOURCES table 13→15, object-safety test extended |
| `crates/flexcore-application/src/forecast_architecture.rs` | NEW — P15.01 |
| `crates/flexcore-application/src/forecast_baselines.rs` | NEW — P15.02 |
| `crates/flexcore-application/src/forecast_pv.rs` | NEW — P15.03 |
| `crates/flexcore-application/src/forecast_evaluation.rs` | NEW — P15.05 |
| `crates/flexcore-application/src/model_registry.rs` | NEW — P15.06 |
| `crates/flexcore-application/src/lib.rs` | MODIFIED (additive) — five `pub mod` declarations with recorded headers + one `pub use` block |
| `crates/adapters/protocol-entsoe/` | NEW crate — P15.04 (`Cargo.toml`, `src/{lib,document,xml_seam,token,transport,scope}.rs`, `tests/a44_extraction.rs`) |
| `Cargo.toml` (root) | MODIFIED (additive) — member entry with placement note + workspace dependency entry |
| `tools/check_dependency_rule.py` | MODIFIED (additive) — `COMPOSITION_CRATES` + `MEMBER_DIRS` entries for `flexcore-entsoe-adapter` |
| `apps/worker/python/forecast_worker/` | NEW — P15.02 worker skeleton (`README.md`, `forecast_baseline_worker.py`, `tests/test_forecast_baseline_worker.py`) |
| `benchmarks/forecast-eval-set-v1.json` | NEW — the P15.05 frozen eval set (P8.19 shape mirrored) |
| `docs/phases-11-20/phase-15-forecasting-asset-intelligence/records/01..07` | NEW — the seven records including this one |

## 2. Permitted verification tools — outputs VERBATIM

Run at the end of the batch (Git Bash, Windows; the mojibake in the dependency-rule line is the console's rendering of the checker's own em-dash and is quoted exactly as emitted):

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
(exit 0)

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
(exit 0)

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 28 workspace members.
(exit 0)
```

No other tool was run; no cargo/pip/pytest command was executed (loop bans).

## 3. Honesty inventory (the phase constraints, checked)

1. **No forecast is ever produced by this program**: the baseline rule sets are arithmetic definitions; the worker was never executed; no artifact exists.
2. **Every evaluation cell EMPTY**: `benchmarks/forecast-eval-set-v1.json` carries `run_status: NOT_RUN` and `observed_value: null` in every cell; the program-side record type is unconstructable (no public gate constructor).
3. **Every model registers UNTRAINED**: the registry's `TrainingState::Untrained` is the state of every entry this delivery could register; `Trained` requires a training-data reference and is unreachable without a human-gated run.
4. **No skill claim anywhere**: grep-level absence of accuracy/skill figures in the worker; the `NO_SKILL_STATEMENT` and the envelope `no_skill_note` state the standing no-skill default.
5. **The never-granted permission set stays byte-identical** — quoted verbatim as mirrored in `crates/flexcore-application/src/forecast_architecture.rs::NEVER_GRANTED_PERMISSIONS_MIRROR` and `apps/worker/python/forecast_worker/forecast_baseline_worker.py::NEVER_GRANTED_PERMISSIONS`, both pinned byte-identical to `crates/flexcore-kernel/src/manifest.rs::NEVER_GRANTED_PERMISSIONS` by WRITTEN-UNRUN tests:

   ```text
   dispatch.authorize
   dispatch.issue
   secret.read
   network.any
   ```

   The kernel file itself was NOT edited.
6. **ADR-010 extension, structural**: no function in the new modules constructs or carries a dispatch command; the producer permission vocabulary (`telemetry.read`, `forecast.publish`) has no dispatch member; the price port and adapter have no bid/offer/order surface.
7. **No new external dependencies**: the adapter crate is dependency-free; the Python worker is stdlib-only with guarded imports; workspace pins untouched.
8. **No placeholder**: grep for `todo!/unimplemented!/TODO/FIXME` over every new file returns clean.

## 4. Deviations and recorded decisions

1. **A new crate was created** (`flexcore-entsoe-adapter`) where the plan proposed "modules in an existing adapter crate per conventions §6.2": the A44 model types, extraction seam, token gate, and query assembly are real code with clear ownership and a stable dependency direction (the conventions §6.2 test); the checker and root manifest were updated additively and the dependency rule holds for the resulting 28 members.
2. **Records 01–07 numbering**: one record per subtask (01–06) plus this close-out (07), per the wave assignment; P15-E-1 is registered in record 03 and P15-E-2 in record 04, both OPEN with DEFAULTs. P15-E-3 belongs to Agent B's scope and was NOT registered here.
3. **The typed (non-JSON) provenance mirror** in `forecast_architecture.rs` is the recorded consequence of the application crate's no-serialization-library policy (the JSON record path remains the research crate's pattern; the composition root may parse JSON and call the constructor).
4. **Threshold placeholder convention**: the eval-set v1 registers bound 0.0 = "no threshold registered yet" (recorded in the artifact's `registered_bound_note` fields and in record 05); a real bound is a NEW set version.
5. No checkbox was ticked anywhere; the ledger, plan.md, and context.md were not edited.

## 5. Handoffs

**To Agent B (wave 1)**: no conflicts — Agent B's ownership (`crates/flexcore-domain/src/` extensions, `scheduling_objective.rs`, optimizer data-contract wiring) does not intersect the files above; the shared `crates/flexcore-application/src/lib.rs` was re-read before this agent's additive edit (no B-side application change was present at edit time; if B appends after this record, the additive-append protocol holds). `crates/flexcore-contracts/src/units.rs` was read-only for this agent and untouched.

**To wave 2 (Agent A)**: P15.16 consumes the `envelope_staleness.rs` trigger vocabulary and the P15.02 baselines as the fallback source; P15.17 completes the `forecast_worker/` skeleton (the request/response contract and the permission gate are already fixed); P15.18's manifest v2 must keep the EMPTY-until-human-gate discipline for any forecast dataset. The eval-set growth path (new windows over runtime data) is `forecast-eval-set-v2` as a NEW file.

**To the human gates**: the first build/test pass of the workspace (including the new 28th member and the Python worker's self-tests); P15-E-1 and P15-E-2 (vendor selection, budgets, jurisdiction entries — human-closers); the UNSIGNED token registration; the lab runtime's `EvaluationRunGate` constructor when the first evaluation run is authorized.
