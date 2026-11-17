# P15.18 — Evaluation datasets policy (training-data manifest v2): dataset registration discipline, no production data in simulation, EMPTY rows until the human generation gate, fail-closed scope checks extended to forecasting datasets

| Field | Value |
|---|---|
| Subtask | P15.18 (training-data manifest v2: dataset registration discipline, no production data in simulation — the Phase 8/9 discipline carried, EMPTY rows until the human generation gate, fail-closed scope checks extended to forecasting datasets) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the manifest is a NEW versioned file, never an edit of v1. NO dataset exists: every per-instance row is EMPTY, no generation run happened, no evaluation cell is filled, every model registers UNTRAINED. |
| Sources | the owning Phase 8 record `ledger/phase-08-advanced-coordination/35_training_data_provenance.md` (the P8.35 standard this manifest extends); the v1 manifest `apps/worker/python/training-data-manifest-v1.json` (carried BY REFERENCE); the P15.05 eval-set freeze (`benchmarks/forecast-eval-set-v1.json`, record 05) |
| Files touched | `apps/worker/python/training-data-manifest-v2.json` (NEW — a NEW file; v1 untouched) |

## 1. What landed

| Item | Content |
|---|---|
| Manifest v2 (`flexcore-research/training-data-manifest.v2`) | NEW versioned file; the v1 GNN topology-family assignment is carried forward BY REFERENCE (never restated, never edited); v2 adds ONLY the dataset-registration surface below |
| Registration discipline | The required fields EVERY dataset (GNN or forecasting) carries at generation time, extending the P8.35 per-dataset provenance standard to forecasting: dataset_id/version, closed dataset_kind (`gnn_topology` / `forecast_eval` / `forecast_training`), simulator-or-synthetic source with generator version, size, license + consequence, leakage controls, digest rule; an unfilled field is recorded `unavailable: <reason>`, never omitted, never guessed |
| No production data in simulation | The Phase 8/9 rule carried verbatim: the ONLY permitted dataset sources in 0.x are (a) simulator-generated series over the declared scenario library and (b) the labeled P3.31 synthetic fixture; a row whose source is neither is INVALID and must not be entered (fail-closed) |
| EMPTY rows | Both registered forecasting families (`fds-forecast-eval-synthetic-fixture`, `fds-forecast-training-simulated-series`) are REGISTERED STRUCTURES with EMPTY rows, filled only at the human/lab generation gate; the forecast_training registered scope is EMPTY — any consumer naming a training dataset flags out-of-scope (fail-closed by design) |
| Fail-closed scope checks | The P8.11 discipline EXTENDED to forecasting datasets: EXACT string equality on dataset/instance ids (a near-miss flags, never passes); a worker request naming an unregistered dataset is refused with a named error (no default dataset, no degraded-mode forecast); the scope check runs at consumption time and the reviewer re-checks |
| Eval-set growth rule | `forecast-eval-set-v2` is a NEW FILE with a NEW registered window/threshold set (X-8: thresholds registered BEFORE any run that could fill them); the v1 artifact is never edited (the P15.05 frozen-artifact rule, restated in the manifest) |

## 2. Recorded decisions

1. **A new file, never an edit of v1** (the brief's hard rule and the P8.19 anti-silent-growth rule agree): `supersedes` names v1 by path; a v1 content dispute is a NEW manifest version.
2. **No code was added to `gnn_worker/`** (outside this agent's ownership): the fail-closed scope check for forecasting datasets is a manifest-level rule (the registration + consumption contract) recorded here; a future enforcement implementation lands with the forecasting-worker composition root.
3. **ADR-010 note in the manifest**: no dataset, eval result, or trained model authorizes or produces a dispatch command; the never-granted permission set is untouched.

## 3. Honest boundaries

1. No dataset exists; no per-instance cell is filled; no generation run happened; the poisoning-detection run remains a HUMAN GATE, UNRUN (carried unchanged).
2. Every model registers UNTRAINED; every evaluation cell belongs to the P15.05 framework and is EMPTY.
3. The permitted-tool outputs are quoted in the close-out record (48).
