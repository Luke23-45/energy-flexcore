# Phase 15 — Forecasting and Asset Intelligence: Exit-Gate Checklist

**Exit gate (plan §6, the phase's own acceptance criteria): every subtask
has a decision record; the deliverables are DESIGNS, domain-model
specifications, and test fixtures WRITTEN-UNRUN — no forecast is ever
produced by this program's implementation, no skill is claimed anywhere,
every evaluation cell stays EMPTY, all models register UNTRAINED; the
optimizer data contract is extended ONLY as linear/piecewise-linear input
data with the solver-class guard tests written; no forecast, degradation
price, or health score has dispatch reach and the never-granted permission
set is byte-identical; no production data enters simulation; P15-E-1,
P15-E-2, and P15-E-3 remain OPEN with DEFAULTs; the dependency checker
stays green; `verify_docs.py` and `validate_agent.py` report no failures.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
trained, evaluated, ingested, scored, stored, migrated, or produced** —
the first human build is the program gate; every status below is a
written-artifact status, never a runtime claim.

## Plan §6 acceptance criteria (one-for-one)

- [x] **Criterion 1 — every subtask has a decision record citing the
  files touched and the [S-n] sources for every external claim.**
  Evidence: records 01–06 + 07 (wave 1 A), 21–25 + 26 (wave 1 B),
  41–47 + 48 (wave 2 A), 61 (wave 2 B, the decision register record)
  + 62 (the close) exist under `records/`. The ledger's P15.01–P15.11
  ticks carry citations that all resolve to files on disk (close
  record §3.1 — 11 of 11 EXIST); the P15.12–P15.20 ticks are the
  ORCHESTRATOR's post-close action, expected to cite records 41, 42,
  43, 44, 45, 46, 47, 61, and 62. Status: **satisfied by written
  artifacts**.
- [x] **Criterion 2 — the deliverables are designs, domain-model
  specifications, and written-unrun test fixtures; no forecast is
  produced, no skill claimed, every evaluation cell EMPTY, every model
  UNTRAINED.** Evidence: the honesty sweep (close record §3.4) over
  every phase artifact; `benchmarks/forecast-eval-set-v1.json` carries
  `run_status: "NOT_RUN"` in all 16 status fields and `observed_value:
  null` in all 6 result cells (close record §3.4 quotes the counts);
  `EvaluationRunGate` has no public constructor; the registry's
  `TrainingState::Untrained` is the state of every model the delivery
  could register. Status: **satisfied by the audit** as of the close
  record's date (an inspection, not a tool run).
- [x] **Criterion 3 — the optimizer data contract is extended ONLY as
  input data; the solver-class guard tests prove the MISOCP refusal
  and tier guards stay; no new problem class exists.** Evidence: the
  four P15.08 guard tests in
  `crates/flexcore-optimization-api/src/problem.rs` (`p15_08_degradation_data_keeps_the_lp_an_lp`,
  `p15_08_degradation_data_keeps_the_milp_a_milp`,
  `p15_08_degradation_data_does_not_widen_the_class_when_cones_are_added`,
  `p15_08_degradation_data_does_not_relax_the_tier3_refusal`) exist on
  disk and assert (close record §3.5 quotes the names); the convexity
  gate refuses non-convex piecewise data at the DATA contract; no
  `ProblemClass`, tier, or provider type was edited. Status:
  **satisfied by the quoted evidence** (the tests are WRITTEN-UNRUN;
  their execution is the human gate).
- [x] **Criterion 4 — no forecast, degradation price, or health score
  has dispatch reach; the never-granted permission set is
  byte-identical.** Evidence: `git status --porcelain` over
  `crates/flexcore-kernel/src/manifest.rs` returns EMPTY (byte-identical
  to HEAD; close record §3.6 quotes the set verbatim); the forecast
  producer vocabulary is `["telemetry.read", "forecast.publish"]` (the
  forecast-worker manifest, unchanged); the claim-verb and
  dispatch-reach sweeps over every new module return only negated or
  boundary statements. Status: **satisfied by the quoted evidence**.
- [x] **Criterion 5 — no production data enters simulation; the
  training-data manifest v2 keeps the EMPTY-until-human-gate
  discipline.** Evidence: `apps/worker/python/training-data-manifest-v2.json`
  is a NEW file (v1 untouched), both forecasting families are
  registered structures with EMPTY rows, the only permitted 0.x
  sources are simulator/synthetic, and fail-closed scope checks are
  extended to forecasting datasets (record 47 §1). No production
  dataset is referenced anywhere in the phase. Status: **satisfied by
  the audit**.
- [x] **Criterion 6 — P15-E-1/E-2/E-3 remain OPEN with DEFAULTs; the
  dependency checker stays green; `verify_docs.py` and
  `validate_agent.py` report no failures.** Evidence: the register
  verification (record 61 §1 — all three OPEN in plan §4, the ledger
  register, and the owning records); the three permitted-tool outputs
  quoted verbatim in close record §2 (28 workspace members; the two
  doc tools report no failures). Status: **satisfied by the quoted
  evidence**; no agent closed any decision.

## Human-gate inputs still pending (none of them closable by an agent)

| # | Gate input | Unblocks |
|---|---|---|
| 1 | First human `cargo build --workspace` / clippy / `cargo test --workspace` (the new 28th member, the five new domain/application modules, the optimizer wiring, the storage/adapter additions, and every written-unrun test file), plus the Python worker self-tests/pytest suites and migration 0021's apply/down cycle | every WRITTEN-UNRUN claim in the phase |
| 2 | The [S-111] ENTSO-E wire-level verification pass (record 04 §4 — six UNVERIFIED register items) against the API guide with a registered token; the [S-169]/[S-170] category-vocabulary check against the IEEE 1547-2018 text (record 44 §2.4) | the UNVERIFIED protocol/certification details; the recorded edits a mismatch triggers |
| 3 | Human closers for P15-E-1 (budget + vendor selection), P15-E-2 (vendor selection + jurisdiction entry), P15-E-3 (fleet-composition review + per-chemistry parameters) — record 61 §2 names the closers and what each closure unblocks | the three OPEN decision points and the integrations they gate |
| 4 | The X-8 evaluation path: the `EvaluationRunGate` constructor minted in the human/lab runtime (record 05 §3 item 3), real thresholds registered as `forecast-eval-set-v2` BEFORE any run, observed values entering gated evaluation records only | every EMPTY evaluation cell; any future skill claim |
| 5 | The dataset-generation human gate (record 47) — simulator/synthetic generation runs authorized by a human before any manifest-v2 row is filled; the poisoning-detection run (carried UNRUN from Phase 8) | the manifest v2 forecasting rows; any training data |
| 6 | Production SHA-256 digests + managed signing identities for forecast artifacts, registry entries, and warranty evidence packs (records 01, 06, 25, 46 — the test-grade FNV-1a labels stay test-grade) | release-grade provenance; the UNSIGNED postures |
| 7 | The first site/runtime data before any baseline can be validated against reality (plan §5 risk 1): persistence/seasonal-naive/degree-day stay frozen, no-skill references until then | the P3.10 disclaimer supersession rule (evaluated baselines only) |
| 8 | The orchestrator's ledger ticks for P15.12–P15.20, citing records 41–47, 61, and 62, plus the ledger status-line update | the expected 20/20 end state (close record §1) |

Full close position and the consistency-audit results:
`records/62_p15_20_phase_close.md`.
