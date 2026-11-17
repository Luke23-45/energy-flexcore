# Final Review R4 — Python Numerical Workers (Iteration 1)

**Reviewer:** Agent B (wave 2)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 2 · P1: 4 · P2: 6

## 1. Scope reviewed

Full census of `apps/worker/python/` (53 files, ~23,600 LOC of Python).

| Package | Files | LOC (py) | Depth |
|---|---|---|---|
| root (`README.md`, `requirements.txt`, `grid_worker.py`, 2 training-data manifests) | 5 | 366 py + 220 JSON | deep read (grid_worker, manifests) |
| `forecast_worker/` | 4 (1 py + 2 test py + README) | 1,312 + 479 | deep read (worker + Rust mirror cross-check) |
| `gnn_worker/` | 7 | 2,281 + 1,360 tests | deep read (worker + evaluation), tests structural |
| `marl_worker/` | 9 | 3,808 + 2,073 tests | deep read (worker, safety_filter, shadow_log, evaluation_harness core) |
| `andes_worker/` | 7 | 2,739 + 1,196 tests | deep read (worker + evaluation_harness metrics), cosim_crosschecks targeted |
| `portfolio_analytics/` | 3 | 866 + 610 tests | deep read (analytics core) |
| `drill/` | 2 | 1,020 + 349 | structural skim + targeted deep reads (fault vocabulary, transcribed constants) |
| `reproducibility/` | 6 | 1,776 + 662 tests | deep read (all three modules) |
| `helics_federates/` | 8 | 1,646 | deep read (federate_common, grid_federate, broker, asset/control targeted) |

Cross-boundary Rust files read for contract verification only (out of scope for
defects): `crates/flexcore-grid-api/src/worker_backend.rs`, `backend.rs`,
`port.rs`; `crates/flexcore-application/src/forecast_baselines.rs`,
`forecast_architecture.rs`; `apps/sim-runner/src/helics_orchestration.rs`;
`apps/worker/src/supervision.rs`; `crates/flexcore-research/src/validation.rs`.

## 2. Method

Static, read-only (AGENTS.md §6 tools not applicable to code; no
install/build/run, per the mission bans):

1. **File census** (`find` + `wc -l`) — table above.
2. **Import-vs-definition sweep** — every `from X import Y` / `import X` in the
   tree was matched against the target module's definitions (grep of `^def`,
   `^class`, `^CONST`). Result: all cross-module imports resolve; deferred
   framework imports (`torch`, `torch_geometric`, `ray`, `gymnasium`, `andes`,
   `numpy`, `helics`, `pandapower`) are all guarded inside named functions by
   design (the written-unrun honesty gates), verified individually.
3. **Arity/keyword cross-checks** on every cross-module call found in the sweep
   (see §3 items 5–7). All matched.
4. **Requirements-vs-imports audit** — third-party imports in non-test code:
   `pandapower` (pinned 3.5.6 in `requirements.txt`, matches
   `PINNED_PANDAPOWER_VERSION`), `helics` (pinned 3.7.0, matches
   `PINNED_HELICS_VERSION`), `andes` (pinned 2.0.0, matches
   `ANDES_PINNED_VERSION`), `torch`/`torch_geometric`/`ray`/`gymnasium`/`numpy`
   (deliberately unpinned — recorded DECISION-POINTs, each with a written
   requirements-file rationale). Test-only import: `pytest` — **not recorded
   anywhere** (finding FRV1-R4-006).
5. **Rust↔Python contract tables** (see §3 items 1–4): protocol tags, JSON
   field names both directions, request/response shapes, transcribed constants,
   NATS subjects (none exist on the Python side — the Python workers are
   file/stdin-seam processes; NATS lives entirely in `apps/worker/src`, so
   there is no Python-side subject string to mismatch).
6. **Hand-computed self-test arithmetic spot-verification** — re-derived the
   stated expectations for forecast (persistence/seasonal wrap indices,
   degree-day orthogonal fit 1/2/3, FNV-1a published vectors), gnn (ranking
   order, tie-break, screening metrics 1/2, 0.5, 7.0), marl (clamp 7.0→3.5,
   infeasible interval [2.5, 2.0], rates 0.2/0.2/0.6), andes (nadir/settling/
   ROCOF/damping definitions), portfolio (0.25 shortfall, 0.95 ratio, 2/3
   coverage), reproducibility (FNV vectors, XOR identity, boundary-inclusive
   0.25 tolerance), drill and cosim (spot). Two expectation/implementation
   mismatches found (FRV1-R4-001, FRV1-R4-002).
7. **Honesty sweep** — grep + read for skill/accuracy/speedup claims,
   solver-class terms (SOCP/MISOCP misuse), certification/safety language,
   UNTRAINED/no-skill labeling, proposal-only scans, mode locks. One stale
   constant vs the training-data manifest (FRV1-R4-008); otherwise clean.
8. **README accuracy** — read all 8 package READMEs + the top-level README.

## 3. Checks performed, all consistent

1. **grid_worker.py ↔ `worker_backend.rs`**: protocol tag
   `flexcore-grid-worker/1` = `WORKER_PROTOCOL_ID`; request kind
   `run_power_flow` = `WORKER_REQUEST_RUN_POWER_FLOW`; model IR fields
   (`model_ref.model_id`, `base_mva`, `buses[].bus_id/nominal_kv`,
   `branches[].branch_id/from_bus/to_bus/r_ohm/x_ohm/rated_current_a`,
   `transformers[].transformer_id/hv_bus/lv_bus/rated_mva/vk_percent/
   vk_r_percent`, `external_grids[].bus_id/vm_pu/va_degree`) all match
   `port.rs` serde shapes; injection fields
   (`injection_id/bus_id/p_injection_mw/q_injection_mvar`) match
   `BackendInjection`; all 9 outcome fields (`converged`, `reported_error`,
   `provider_version_reported`, `bus_results[].bus_id/vm_pu`,
   `branch_results[].branch_id/i_from_ka/i_to_ka/loading_percent`,
   `transformer_results[...]`, `iterations`, `worker_status_text`,
   `warnings`) match `BackendPowerFlowOutcome`; the Python response
   `{"protocol": ..., **outcome}` matches the Rust `#[serde(flatten)]`
   envelope. PASS.
2. **forecast_worker ↔ `forecast_baselines.rs`**: the three frozen baseline
   ids (`forecast-baseline/persistence|seasonal-naive|degree-day-regression`)
   and versions (`persistence.v1`, `seasonal-naive.v1`, `degree-day.v1`)
   match the Rust constants verbatim; the seasonal-naive algorithm
   (index `n - season_length + (step % season_length)`, wrap rule, minimum
   history `season_length + 1`, refusal texts) is identical to the Rust
   implementation. PASS.
3. **forecast_worker publication seam ↔ `forecast_architecture.rs`**:
   `PROVENANCE_SCHEMA_ID_MIRROR` = `FORECAST_PROVENANCE_SCHEMA_ID`
   (`flexcore-forecast-artifact-provenance-v1`);
   `DIGEST_ALGORITHM_FNV1A64_TEST` = `FORECAST_DIGEST_ALGORITHM_FNV1A64_TEST`
   (`fnv1a64-test`); the FNV-1a 64 constants and the three published test
   vectors match the Rust `fnv1a64`. (The canonical-string *field list* is
   intentionally different — the Python one canonicalizes a publication
   record, the Rust one a baseline-definition record; both cite the same
   pattern, not the same artifact. Not a mismatch.) PASS.
4. **helics federates ↔ `helics_orchestration.rs`**:
   `flexcore-helics-federation/1` and `flexcore-helics-federate/1` match
   `HELICS_FEDERATION_PROTOCOL_ID` / `HELICS_FEDERATE_REPORT_PROTOCOL_ID`;
   every `federate_report` field name matches `FederateExitReport` (optional
   fields carried as `null` for the broker report); `CANONICAL_UNITS`
   (8 units) matches the Rust `CANONICAL_UNITS` array;
   `federation_config.example.json` satisfies every option the three
   federate scripts require (`grid_worker_script`, `model_ir_path`,
   `bus_map`, `input_records_path` + `asset_key_map`, `setpoints_path`,
   topic/unit forms); the grid federate speaks the exact
   `flexcore-grid-worker/1` request shape of the P5.15 worker. PASS.
5. **marl_worker cross-module calls**: `_log_entry_for` kwargs match
   `shadow_log.build_log_entry`'s keyword-only signature exactly;
   `filter_action` report keys (`action_kind`, `proposed_permission`,
   `action_outcome`, `entries[].{asset_id, entry_outcome, p_current_mw,
   p_proposed_mw, p_filtered_mw}`, `filter_activations`, `infeasibility`)
   match every consumer in `marl_worker.py` and
   `evaluation_harness.filter_statistics`; `shadow_log.log_summary` provides
   every key `filter_statistics` reads. PASS.
6. **marl test imports**: `from evaluation_harness import FILTER_OUTCOMES,
   REFUSING_OUTCOMES` resolves (re-exported from `safety_filter` at module
   level). PASS.
7. **andes_worker ↔ `evaluation_harness`**: `import evaluation_harness`
   resolves; `evaluation_harness.QUANTITIES` exists; the
   `evaluate_response(times, values, *, baseline, event_time, tolerance,
   quantity)` call site matches the definition keyword-for-keyword. PASS.
8. **gnn_evaluation ↔ gnn_inference_worker**: `BOUNDARY_STATEMENTS`,
   `GNN_PIN_STATUS`, `rank_candidates`, `evaluate_topology_scope` all exist;
   the module-level import is pure (torch/PyG guarded inside functions).
   PASS.
9. **Transcribed Rust constants**: `SUPERVISION_DEFAULT_CALL_TIMEOUT_MS =
   30_000` matches `supervision.rs:102 DEFAULT_CALL_TIMEOUT_MS: u64 =
   30_000`; `MAX_PROPOSAL_PAYLOAD_BYTES = 1024 * 1024` matches
   `validation.rs:258`. PASS.
10. **Training-data manifests ↔ seed_derivation.py**: the derivation string
    `seed(asset_key) = master_seed XOR splitmix64(FNV-1a-64(asset_key))` and
    master seed `20260601` in `benchmarks/research-set-v1.json` match
    `RECORDED_SEED_DERIVATION` / `RECORDED_MASTER_SEED` verbatim; manifest
    v2 carries v1 forward strictly by reference. PASS.
11. **NATS subjects**: no NATS subject string exists in any Python file (the
    workers are one-JSON-document stdin/stdout seams); the NATS vocabulary
    is Rust-side only. No cross-side subject to mismatch. PASS.
12. **Self-test arithmetic** (beyond the two mismatches in §4): forecast
    seasonal wrap indices, degree-day orthogonal fit (1.0/2.0/3.0), FNV
    published vectors; marl clamp 7.0→3.5 and infeasible [2.5, 2.0] interval
    arithmetic; gnn ranking/tie-break/screening metrics; portfolio
    shortfall/ratio/coverage; reproducibility boundary-inclusive tolerance
    and XOR identity; andes metric definitions — all hand-re-derived and
    consistent. PASS.

## 4. Findings

### FRV1-R4-001 [P0] [phase-15/phase-08] forecast/gnn/marl `__main__` blocks never call `main()` — the documented transport contract cannot run

- Status: OPEN
- Location:
  - `apps/worker/python/forecast_worker/forecast_baseline_worker.py:1306-1312` (`if __name__ == "__main__": _records = run_self_tests()`)
  - `apps/worker/python/gnn_worker/gnn_inference_worker.py:1560-1566` (same pattern)
  - `apps/worker/python/marl_worker/marl_worker.py:1439-1445` (same pattern)
  - Contrast (correct): `apps/worker/python/grid_worker.py:365-366` and
    `apps/worker/python/andes_worker/andes_dynamic_worker.py:699-700` both wire
    `main()`.
- Issue: All three workers define the full stdin→stdout composition edge
  (`def main()` at forecast:885, gnn:1016, marl:861) but the `__main__` block
  runs the in-module self-tests and exits, never calling `main()`. The
  documented transport contract cannot be exercised as written:
  `forecast_worker/README.md:74-76` instructs
  `python apps/worker/python/forecast_worker/forecast_baseline_worker.py < request.json`
  — that invocation prints self-test PASS lines and exits per the self-test
  result, never reading the request. The same README text exists for the gnn
  and marl workers' composition edges. This is the would-not-run class: the
  first human execution of the documented seam produces the wrong program
  behaviour entirely (and, worse, the self-test output on stdout violates
  each README's "malformed input → NOTHING on stdout" framing).
- Recommendation: Minimal fix, consistent with grid_worker/andes: dispatch on
  argv in the `__main__` block (e.g. `--self-test` runs `run_self_tests()`,
  no-args runs `main()`), or move the self-test entrypoint to a documented
  flag; update the three READMEs' self-test commands accordingly. One-line
  semantic change per file; no behaviour of the tested functions changes.

### FRV1-R4-002 [P0] [phase-15/phase-08] `"approved"` key is not in `FORBIDDEN_RESULT_KEYS`, but the self-test/pytest suite expects its refusal — the human gate fails on first execution

- Status: OPEN
- Location:
  - `apps/worker/python/gnn_worker/gnn_inference_worker.py:1246-1253` (self-test 12 expects a RuntimeError for a nested `"approved"` key; the detail text asserts "a nested 'approved' key anywhere in the tree refuses emission")
  - `apps/worker/python/forecast_worker/tests/test_forecast_baseline_worker.py:197-206` (`test_proposal_only_refuses_a_nested_forbidden_key` uses `"approved"` and raises AssertionError when no refusal comes)
  - The sets (identical, none contains `"approved"`):
    `forecast_worker/forecast_baseline_worker.py:124-139`,
    `gnn_worker/gnn_inference_worker.py:189-204`,
    `marl_worker/marl_worker.py:214-230`,
    `andes_worker/andes_dynamic_worker.py:147-162`,
    `andes_worker/cosim_crosschecks.py:127`.
- Issue: `assert_proposal_only` checks `key.strip().lower() in
  FORBIDDEN_RESULT_KEYS`; `"approved"` is not a member of any of the five
  forbidden-key sets (they contain `"approval"` but not `"approved"`). Both
  affected tests therefore record a failure: the gnn in-module self-test
  appends `passed: False` (→ exit 1 from `__main__`), and the forecast pytest
  test raises `AssertionError`. The documented human build/test gate fails on
  its first run. The root cause is a real boundary question, not a test typo:
  `"approved"` is authorization-shaped and arguably *should* be forbidden —
  the test encodes the intended boundary; the sets are missing the key.
- Recommendation: Add `"approved"` to `FORBIDDEN_RESULT_KEYS` in all five
  modules that carry the set (one-token edit each, keeping the sets aligned),
  or — if the set is deliberately closed to `"approval"` — change the two test
  inputs to use `"approval"` and record the closed-set decision. The first
  option is the stronger boundary and matches the tests' stated intent.

### FRV1-R4-003 [P1] [phase-08] GNN torch path builds `edge_index` with the wrong layout for the documented PyG API

- Status: OPEN
- Location: `apps/worker/python/gnn_worker/gnn_inference_worker.py:932-935`
- Issue: The code flattens the edge list row-major and reshapes:
  `torch.tensor([e for edge in edges for e in edge]).reshape(2, -1)`. For
  edges `[[0,1],[1,2]]` this produces the flat list `[0,1,1,2]` → tensor
  `[[0,1],[1,2]]` — i.e. row 0 = `[s0, d0, …]` interleaved, not the PyG
  `edge_index` convention (row 0 = all sources, row 1 = all destinations).
  For any graph with ≥ 2 edges the layout is wrong (for 2 edges it happens to
  look plausible, which makes the error insidious), and every message-passing
  result downstream would be computed on a corrupted graph. The header
  declares every torch/PyG call UNVERIFIED, but this is a demonstrable shape
  error against the documented API the header claims to follow ("follows the
  documented PyG message-passing API shape").
- Recommendation: Replace with the documented form, e.g.
  `torch.tensor([[e[0] for e in edges], [e[1] for e in edges]], dtype=torch.long)`
  or `torch.tensor(edges, dtype=torch.long).t().contiguous()`, and add a
  hand-computed shape expectation to the human-gate notes.

### FRV1-R4-004 [P1] [phase-08] `rerun_check.rerun_verdict` never checks that the injected tolerance's class matches the primary record's declared `tolerance_class` — the outcome can be mislabeled `REPRODUCED_EXACT` under a non-exact comparison

- Status: OPEN
- Location: `apps/worker/python/reproducibility/rerun_check.py:341-398`
  (class read at :354, comparison at :365-369, outcome labeling at :377-382)
- Issue: `rerun_verdict` labels the outcome from
  `primary.get("tolerance_class")` (`REPRODUCED_EXACT` when the record says
  `exact`) but performs the comparison exclusively under the caller-injected
  tolerance's class (`compare_outputs`). If the two disagree — e.g. an
  exact-class record rerun with an injected `{"class": "within_tolerance",
  "value": 100}` tolerance — nonzero deviations are accepted by the comparison
  and the verdict is still emitted as `REPRODUCED_EXACT`. Header item 5
  promises "exactness is a declared class, not an assumption"; as written, a
  caller-side tolerance/class mismatch silently defeats it. No self-test
  covers the conflicting-class case.
- Recommendation: Fail closed on a declared-vs-injected class conflict (a
  named refusal, e.g. `TOLERANCE_CLASS_CONFLICT`, mirroring the existing
  refusal discipline), or at minimum derive the outcome label from the
  injected tolerance class and record the conflict in `findings`. Add one
  self-test pinning the refusal.

### FRV1-R4-005 [P1] [phase-05] Top-level `apps/worker/python/README.md` documents only the grid worker — the directory now contains eight packages and two manifests it does not mention

- Status: OPEN
- Location: `apps/worker/python/README.md:10-15` (Contents table lists only
  `grid_worker.py` and `requirements.txt`)
- Issue: The directory has grown to `forecast_worker/`, `gnn_worker/`,
  `marl_worker/`, `andes_worker/`, `portfolio_analytics/`, `drill/`,
  `reproducibility/`, `helics_federates/`, plus
  `training-data-manifest-v1.json` / `-v2.json`. The top-level README's
  Contents table and its body ("the Python leg of the P5.15 grid-simulator
  adapter integration") describe only the Phase 5 grid worker. A release
  reader landing on `apps/worker/python/` gets a materially incomplete map of
  the area (checklist item: README accuracy).
- Recommendation: Extend the Contents table with one row per package and one
  row per manifest (each with its owning phase pointer — P5.15, P5.16,
  P8.x, P12.09, P15.02/15.17); keep the grid-worker honesty text as the
  entry-point section for `grid_worker.py` specifically.

### FRV1-R4-006 [P1] [phase-08/09] `pytest` is required by every package's documented human gate but is recorded in no requirements file

- Status: OPEN
- Location: all 10 `tests/` directories import `pytest` (e.g.
  `forecast_worker/tests/test_forecast_baseline_worker.py` — note this one
  additionally relies on `importlib`; `reproducibility/README.md:36-37` and
  `forecast_worker/README.md:96` document `pytest` invocations as the human
  gate); `apps/worker/python/requirements.txt` pins `pandapower` only; the
  per-package `requirements-*.txt` files deliberately carry no test tooling.
- Issue: The repository's dependency discipline is "recorded pins only, never
  installed in-loop". Framework deps (torch/PyG/ray/andes/helics) are either
  pinned with a source citation or explicitly declared DECISION-POINTs in
  commented requirements files. `pytest` is neither: it is imported by 20
  test modules and named in the human-gate commands of at least two READMEs,
  yet no file records a version pin, a license note, or a decision-point
  statement for it. This is an inconsistency in the pin discipline itself
  and a release-readiness gap: the human gate cannot run as documented
  without an unrecorded dependency.
- Recommendation: Add a short recorded entry — either a pinned
  `pytest==<verified version>` line in `apps/worker/python/requirements.txt`
  (test-only section, with the same D4/license-audit framing as the other
  pins) or a DECISION-POINT comment block mirroring the gnn/marl files'
  style. Do not add an unpinned uncommented line.

### FRV1-R4-007 [P2] [phase-15] forecast README claims "numpy/pandas are guarded imports" — pandas does not exist anywhere in the worker

- Status: OPEN
- Location: `apps/worker/python/forecast_worker/README.md:68-70` (honesty
  line 6) vs `forecast_worker/forecast_baseline_worker.py` (guards `numpy`
  only, `import_numpy` at :199-211; no `pandas` token in the file).
- Issue: README/comment drift. The worker guards exactly one framework
  (`numpy`); `pandas` is never imported, guarded, or mentioned in the code.
- Recommendation: Drop "pandas" from the README line (or reword to "numpy is
  a guarded import …; no other framework is referenced").

### FRV1-R4-008 [P2] [phase-08] `gnn_evaluation.HOLDOUT_STATUS` predates training-data-manifest-v1 — its lib-06/07/08 hold-out assignment is superseded by manifest findings F-1/F-2

- Status: OPEN
- Location: `apps/worker/python/gnn_worker/gnn_evaluation.py:129-136`
  (`HOLDOUT_STATUS` names "held-out families lib-06/lib-07/lib-08") vs
  `apps/worker/python/training-data-manifest-v1.json` finding F-1 (lib-06/07/08
  share the training fixture; they are assigned to the HELD-OUT
  OPERATING-REGIME track, and the topology hold-out is the generator-produced
  families) and F-2 (the meshed lib-03 excluded by default).
- Issue: The constant honestly labels itself "RECOMMENDATION — UNCONFIRMED
  until the P8.35 training-data manifest exists" and defers to the manifest
  as evidence — so this is stale-text drift, not a contradiction. But the
  manifest now *exists* and its confirmed assignment differs from the
  constant's text; a reader of the module alone would plan the wrong hold-out
  protocol (the exact near-duplicate leakage risk F-1 records).
- Recommendation: Update `HOLDOUT_STATUS` to cite manifest v1's F-1/F-2
  resolution (regime-track assignment + generator-family topology hold-out)
  while keeping the "the reviewer's overlap check is the evidence" sentence;
  the self-test that pins the constant's prefix must be updated in the same
  edit.

### FRV1-R4-009 [P2] [phase-08] `MARL_BASELINE_IDS` is a dead constant

- Status: OPEN
- Location: `apps/worker/python/marl_worker/evaluation_harness.py:96-102`
- Issue: Defined (the §13.3 per-module baseline subset) and documented, but
  referenced nowhere in the package or its tests (`compare_case_to_baselines`
  validates against `FROZEN_BASELINE_IDS` only). Dead code invites drift from
  the §13.3 mapping it claims to encode.
- Recommendation: Either use it (e.g. a check that the MARL-comparable
  baseline ids are a subset of the frozen registry) or remove it with a
  pointer to the owning record.

### FRV1-R4-010 [P2] [phase-08] `execution_ms` is carried in every gnn/marl/andes envelope but is hardcoded to 0 on every reachable path

- Status: OPEN
- Location: `gnn_worker/gnn_inference_worker.py:1000-1008` (`execution_ms=0`),
  `marl_worker/marl_worker.py:711, 767` (`execution_ms=0`),
  `andes_worker/andes_dynamic_worker.py:656` (`execution_ms=0`); the
  §9.2 provenance field is documented as a template carry-over.
- Issue: No code path ever measures elapsed time (the inference paths are
  unreachable by the honesty gate, and the reachable paths pass a literal 0).
  The field therefore always reads `0`, which a future reader could mistake
  for a measurement. The infeasibility of measuring today is recorded (the
  timeout enforcement is a VERIFY-BUILD item), but the always-0 field is not
  called out where it is emitted.
- Recommendation: Add a one-line comment at each call site ("always 0 until a
  path that can execute exists; never a measurement") or omit the field from
  the reachable envelopes until a real path exists.

### FRV1-R4-011 [P2] [phase-08] Naming/structure drift in self-test helpers

- Status: OPEN
- Location:
  - `marl_worker/marl_worker.py:1261-1266` — self-test named
    `bad_permission_type` actually exercises a *missing
    `environment_version`* (its detail text says so); the name misleads.
  - `gnn_worker/gnn_inference_worker.py:1444-1446` — `_expects_request_error`
    aliases `check = results.append`, shadowing the enclosing `check`
    closure name; works, but is confusing against the sibling modules'
    identical helpers (which append directly).
  - `gnn_worker/gnn_evaluation.py:708-711` — `__main__` calls
    `run_self_tests()` twice (once for the failure list, once for printing);
    every sibling module computes the list once.
- Issue: Polish-only; no behavioural effect. Inconsistency vs sibling
  packages makes the self-test suites harder to diff-audit.
- Recommendation: Rename the marl test (`missing_environment_version_refused`
  style), delete the aliasing line in the gnn helper, and reuse the single
  `run_self_tests()` result in gnn_evaluation's `__main__`.

### FRV1-R4-012 [P2] [phase-08] ANDES README overstates the metrics path: no trace can ever be exported by the current code, so the harness can never produce metrics even at the human gate

- Status: OPEN
- Location: `apps/worker/python/andes_worker/README.md:17-19` ("returns the
  outputs … with the metrics of the pure P8.17 evaluation harness computed on
  actually extracted traces") vs
  `andes_worker/andes_dynamic_worker.py:476-504` — `extract_traces` marks
  every trace `exported: False` with the diagnostic "signal-to-array mapping
  is a VERIFY-BUILD item … not exported in this run record"; the metrics step
  in `run_study` (:622-644) only runs on exported traces.
- Issue: The in-code VERIFY-BUILD recording is honest and deliberate
  (recorded design decision — not flagged as a defect), but the README
  sentence describes a metrics-on-traces capability that the code cannot
  reach even with ANDES installed, until the signal-mapping work is done. A
  reader of the README alone would expect metrics after pinning ANDES.
- Recommendation: Add one sentence to the README's status/what-it-is section:
  the signal→array mapping in `extract_traces` is itself a VERIFY-BUILD item;
  until it is implemented, `metrics.per_trace` is empty by construction
  (diagnostics record the gap), never fabricated.

## 5. Release-readiness verdict for this area

The Python numerical workers are unusually disciplined for written-unrun
code: every module carries an explicit honesty header, the guarded-import /
decision-point pattern is applied consistently, the proposal-only boundary is
enforced in code (not prose) in all five result-emitting workers, UNTRAINED /
no-skill / research-only framing is present everywhere it must be (forecast
baselines are labeled no-skill references, GNN/MARL/ANDES outputs are
proposals with empty measured cells, solver-class discipline statements are
verbatim and correct, HELICS is coordination-only), and the Rust↔Python
contracts (grid worker, helics federation, forecast mirror constants)
match field-for-field with no drift found. What must be fixed before
release: the two P0s — three of the five worker entrypoints do not wire
their documented stdin/stdout transport (`__main__` runs self-tests
instead), and the `"approved"`-key test/forbidden-set mismatch makes the
documented human gate fail on its very first execution — plus the three
P1s that would mislead the gate's operators (the PyG `edge_index` layout
error, the `rerun_check` class-conflict mislabeling, the stale top-level
README, and the unrecorded `pytest` dependency). All are small, local fixes;
none requires redesign. Optional polish (P2) concerns stale constants and
comment drift that should be corrected opportunistically so the recorded
text keeps pace with the artifacts it cites (manifest v1, the ANDES
VERIFY-BUILD gap). Everything else — import graph, arity/keyword contracts,
transcribed constants, self-test arithmetic, honesty sweep — checked clean
and is release-ready as written.
