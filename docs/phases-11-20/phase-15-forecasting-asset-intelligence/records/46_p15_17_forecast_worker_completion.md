# P15.17 — Python forecasting worker skeleton completion: the publication-record seam

| Field | Value |
|---|---|
| Subtask | P15.17 (complete `apps/worker/python/forecast_worker/`: guarded deps, proposal/record outputs ONLY — no dispatch reach; stdout contract mirroring `gnn_worker` — one JSON in → one envelope out; malformed → NOTHING on stdout; self-tests runnable with nothing installed) |
| Wave / agent | Phase 15 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the completion is written, never executed. No forecast was produced by any run of the worker; every test expectation is hand-computed or a literal copied from the module's own constants. The stdlib-only core still imports and self-tests with NOTHING installed. |
| Sources | [S-41] (forecasting table stakes — the out-of-process execution seam); seams: the wave-1 skeleton (`forecast_baseline_worker.py`, `tests/test_forecast_baseline_worker.py`), `crates/flexcore-application/src/forecast_architecture.rs` (P15.01 provenance layer, mirrored by citation), `apps/worker/python/gnn_worker/` (transport-contract pattern) |
| Files touched | `apps/worker/python/forecast_worker/forecast_baseline_worker.py` (additive completion); `apps/worker/python/forecast_worker/tests/test_forecast_publication.py` (NEW); `apps/worker/python/forecast_worker/README.md` (additive) |

## 1. What landed (additive to the wave-1 skeleton)

| Item | Content |
|---|---|
| Mode `publish_forecast_record` | The second closed mode: the same frozen baseline rule set runs (identical gates), and the result is wrapped as a `forecast_publication_record` envelope — the P15.01 provenance layer mirrored: `schema_id` `flexcore-forecast-artifact-provenance-v1` (mirrored verbatim), the REQUIRED provenance links (`input_data_ref`, `producing_model_ref`, `eval_set_ref`) validated in `validate_request` BEFORE any computation so provenance cannot silently detach, `artifact_kind` from the closed `load` / `pv_generation` / `price` vocabulary, and a whitespace-free `forecast_version` |
| The test-grade digest | FNV-1a 64 over the record's canonical rendering (stable field order, `|` separators — the `ForecastBaselineRecord::canonical_string` pattern mirrored); the Python helper is pinned by the SAME published vectors the Rust helper carries (empty input, "a", "foobar"); loudly labelled `fnv1a64-test` and `TEST_GRADE_DIGEST_STATEMENT`: an integrity stamp for replay comparison, explicitly NOT a security primitive and NOT a signature — the record is `UNSIGNED` (no signing identity exists; a production SHA-256 digest with a managed key is a recorded deployment requirement) |
| Boundary invariants unchanged | Proposal-only (`assert_proposal_only` scans the publication envelope exactly like the baseline envelope), permission gate (never-granted set checked FIRST, producer vocabulary closed), mode-locked to simulation/shadow, malformed request → NOTHING on stdout + reason on stderr + exit 2, in-band rule refusals are `termination_status: "failed"` envelopes (a refusal is a result, never a crash) |
| Tests | `tests/test_forecast_publication.py`: the FNV vectors, the required-links/digest/determinism/content-sensitivity checks, the proposal-only clean scan, the before-computation refusals (missing link, unknown artifact kind, whitespace version), the statement pins, and the subprocess transport contract (including the silent-refusal assertions). The wave-1 baseline suite is untouched |
| Self-tests | `run_self_tests()` extended in-module (the nothing-installed gate): the publication additions run pure, no dependency required |

## 2. Recorded decisions

1. **One worker file, two modes** (additive, the wave-1 contract untouched): a `baseline_forecast` request behaves byte-identically to the wave-1 contract; the publication mode is a strict superset path selected by `request.mode`.
2. **The publication record is integrity-stamped, not signed**: no key reference exists, `signer_key_reference` is `None`, and `signing_status` names the UNSIGNED posture — the P15.01 fail-closed default-verifier posture is not weakened (there is nothing here claiming to be a verified artifact).
3. **A failed publication keeps the baseline failure envelope kind**: an in-band rule refusal is a failed envelope; the publication wrapping applies only on completion (recorded in the module).
4. **The digest input includes the values**: a value change changes the digest (content-sensitivity is test-pinned), and the canonical digest input is carried in the envelope for reviewer reproduction.

## 3. Honest boundaries

1. No output of any run exists; the UNRUN note travels in every envelope.
2. No skill claim anywhere: the publication record carries the same no-skill / UNTRAINED / eval-set-EMPTY honesty block as the baseline envelope.
3. No new dependency: the completion is stdlib-only; numpy remains a guarded import inside `import_numpy` raising the NAMED requirement.
4. Every test is WRITTEN-UNRUN; the permitted-tool outputs are quoted in the close-out record (48).
