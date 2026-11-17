# Final Review Ledger — Phase 08

**Ledger status:** **FULLY FIXED (written) — 8 of 8 subtasks fixed, awaiting human build/test.**

Phase scope: see the implementation ledgers (`ledger/phase-08-*` for phases
00–10, `docs/phases-11-20/phase-08-*` for 11–20). Findings originate in
`../01_reviews/` (citation = the finding ID); subtask IDs are
`FRV1-P08.<xx>`. Ticks read exactly **"fixed, awaiting human
build/test"** — never "tested"/"verified"/"works".

## Fix subtasks

| ID | Finding | Sev | Title | Fix surface | Status |
|---|---|---|---|---|---|
| FRV1-P08.01 | `FRV1-R4-003` | P1 | GNN torch path builds edge_index with the wrong layout for the documented PyG API | Build edge_index row-0-sources/row-1-dests in gnn_inference_worker.py | FIXED (written, awaiting human build/test) — fix wave 3 (pure edge_index_rows helper, row-0-sources/row-1-dests, hand-verified arithmetic, self-test + pytest pin) |
| FRV1-P08.02 | `FRV1-R4-004` | P1 | rerun_verdict never checks the injected tolerance class against the record's declared tolerance_class | Add the class-match check and name the conflict outcome | FIXED (written, awaiting human build/test) — fix wave 3 (named TOLERANCE_CLASS_CONFLICT fail-closed outcome) |
| FRV1-P08.03 | `FRV1-R4-006` | P1 | pytest required by every package's human gate but recorded in no requirements file | Record the pytest pin/decision-point per repo discipline (phase-09 gates cross-noted) | FIXED (written, awaiting human build/test) — fix wave 3 (requirements-dev.txt records pytest==9.2.0 as a SECONDARY-SOURCE PIN; RE-VERIFY AT FIRST INSTALL; referenced by the human-gate sections) |
| FRV1-P08.04 | `FRV1-R4-008` | P2 | gnn_evaluation.HOLDOUT_STATUS superseded by training-data-manifest-v1 findings F-1/F-2 | Update HOLDOUT_STATUS to the manifest state | FIXED (written, awaiting human build/test) — fix wave 3 (HOLDOUT_STATUS leads with the manifest-v1 resolution; the old assignment kept under an explicit SUPERSEDED provenance marker) |
| FRV1-P08.05 | `FRV1-R4-009` | P2 | MARL_BASELINE_IDS is a dead constant | Remove it or put it to use | FIXED (written, awaiting human build/test) — fix wave 3 (dead constant removed; zero uses verified) |
| FRV1-P08.06 | `FRV1-R4-010` | P2 | execution_ms hardcoded 0 on every reachable path | Measure it or remove it from the envelopes | FIXED (written, awaiting human build/test) — fix wave 3 (_elapsed_ms measured via time.perf_counter in all three workers; 'measured diagnostic, never a latency claim') |
| FRV1-P08.07 | `FRV1-R4-011` | P2 | Naming/structure drift in self-test helpers | Normalize helper naming/structure | FIXED (written, awaiting human build/test) — fix wave 3 (helper naming/structure normalized; no behavior change) |
| FRV1-P08.08 | `FRV1-R4-012` | P2 | ANDES README overstates the metrics path | Correct the README claim | FIXED (written, awaiting human build/test) — fix wave 3 (ANDES README states the honest VERIFY-BUILD boundary and the EMPTY-by-construction metrics) |
