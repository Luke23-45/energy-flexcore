# HANDOVER — Phase 8: Advanced Coordination Research

Status: **CLOSED** — 39/39 subtasks implemented (P8.01–P8.39); the §23
Stage 8 exit gate is **NOT HELD** (prepared, not demonstrated — see
`30_retrospective_exit_gate.md` §5). Loop bans held throughout: nothing
installed, nothing built, no tests run, no CI run, no drill run, no
advisory query — everything WRITTEN-UNRUN.

## Blockers carried (human gate inputs)

1. Stages 1–7 gates NOT HELD — all demonstrations/runs/sign-offs pending.
2. Research-stack pins (ANDES, torch/jax if used, grpcio) UNPINNED or
   UNVERIFIED — P8.03/P8.36 own the static audit; nothing installed.
3. Every evaluation/training/benchmark cell is EMPTY until the human/lab
   runs fill it — never fabricated.
4. HELICS 3.7.0 / pandapower 3.5.6 pins carried from Phase 5 (API usage
   UNVERIFIED).

## Wave status

| Wave | Subtasks | Status |
|---|---|---|
| 1a | P8.01–P8.03 | pending |
| 1b | P8.04–P8.06 | implemented, awaiting human build/test (decision records: `04_worker_contracts.md`, `05_06_harness_baselines.md`; WRITTEN-UNCOMPILED — nothing built/run) |
| 2a | P8.07, P8.08 | implemented, awaiting human build/test (decision records: `07_admm_dmpc.md`, `08_admm_operations.md`; WRITTEN-UNCOMPILED — nothing built/run) |
| 2b | P8.10, P8.11 | implemented, awaiting human build/test (records: `10_gnn_warmstart.md`, `11_gnn_inference_worker.md`; torch/PyG DECISION-POINT OPEN — nothing installed/run, all inference cells EMPTY) |
| 3a | P8.09, P8.12 | implemented, awaiting human build/test (evaluation TEMPLATES, all measured cells EMPTY per ENTRY_NOTE boundary 4: `09_admm_evaluation.md`, `12_gnn_evaluation.md`; harness code WRITTEN-UNCOMPILED/UNRUN — Rust `admm_evaluation.rs` never built, Python `gnn_evaluation.py` never executed; §19.4 thresholds still NOT pre-registered; torch/PyG DECISION-POINT OPEN) |
| 3b | P8.13, P8.14 | implemented, awaiting human build/test (records: `13_marl_design.md`, `14_marl_worker.md`; RL framework DECISION-POINT OPEN — nothing installed/run, CBF derivation NOT YET SIGNED and training BLOCKED, all training/inference cells EMPTY; worker `apps/worker/python/marl_worker/` WRITTEN-NEVER-RUN) |
| 4a | P8.15, P8.19 | implemented, awaiting human build/test (records: `15_marl_evaluation.md`, `19_benchmark_cases.md`; benchmark-set artifact `benchmarks/research-set-v1.json` + not-tested list, all run_status NOT_RUN) |
| 4b | P8.16, P8.17, P8.18 | implemented, awaiting human build/test (records: `16_andes_integration.md`, `17_dynamic_evaluation.md`, `18_helics_cosim.md`) |
| 5a | P8.20, P8.21 | done (`20_reproducibility.md` + tooling; `21_topology_research.md`) |
| 5b | P8.22, P8.23 | done (`22_portfolio_analytics.md`; `23_cross_module_comparison.md`) |
| 6a | P8.24, P8.25, P8.36 | done (drill PLAN, run UNRUN; security review by reading — F-1 HIGH open; CVE procedure, lookups NOT-EXECUTED) |
| 6b | P8.26, P8.27, P8.28 | done (paper template; Research workspace + GET-only route; 4 additive CI jobs, fail-closed on pin gaps) |
| 7a | P8.29, P8.34 | done (qualification record — all NOT GRANTED; `proposal_windows.rs`) |
| 7b | P8.31, P8.32, P8.33 | done (`tenant_isolation.rs`, `artifact_signing.rs`, `worker_identity.rs`) |
| 8a | P8.35, P8.37, P8.38 | done (provenance + hold-out manifest; property suites; GPU-nondeterminism standard) |
| 8b | P8.39 | done (`39_cosim_crosschecks.md` + `cosim_crosschecks.py`) |
| 9 | P8.30 (coordinator) | done (`30_retrospective_exit_gate.md`; CHECKLIST 28/28 with evidence annotations; consistency audit + 8-tick wording fix) |

## What was implemented

`crates/flexcore-research` (20th workspace member; serde/serde_json ONLY):
validation harness (sealed `ValidatedProposal`, injected feasibility gate,
12 named refusals), frozen baseline registry (HiGHS LP/QP/MIP — not SOCP;
Clarabel conic — not MISOCP; deterministic priority-list heuristic;
frozen-version discipline), exact two-block ADMM + DMPC rolling-horizon
driver with staleness/fallback, ADMM evaluation harness, tenant isolation
(host-stamped scopes, model-serving register), proposal validity windows
(restart-surviving supersession checkpoints), model-artifact signing
(NoArtifactVerifier fail-closed; revocation register; load gate composing
the P8.31 register), worker identity/mTLS/secrets (references-only, value-
never-echoed scan). 10 source files, 14,710 lines + 3 integration-test
files — ALL WRITTEN-UNCOMPILED. `apps/worker/python/` new directories:
gnn_worker (guarded torch/PyG — DECISION-POINT OPEN), marl_worker (safety
filter, shadow log, training UNREACHABLE), andes_worker (guarded ANDES;
dynamic metrics; cosim cross-checks), portfolio_analytics (reporting-only),
reproducibility (seed derivation, record schema, rerun check), drill
(fault injection) — ≈19,000 lines incl. pytest + hand-written property
suites, ALL WRITTEN-NEVER-RUN. `benchmarks/`: `research-set-v1.json`
(frozen version, all run_status NOT_RUN) + 11-item not-tested list.
`apps/worker/python/training-data-manifest-v1.json` (hold-out families
pinned). Console `ResearchWorkspace` + control-plane GET-only
`/api/v1/research/status`. CI: 4 additive research jobs. 38 decision
records in this directory.

## How to build and test it / Blockers / Decisions still needed / Assumptions / Known risks

**Build/test (the human's gate pass — nothing here has been compiled or
executed):** `cargo build --workspace` → `cargo clippy --workspace
--all-targets -- -D warnings` → `cargo test --workspace` (incl.
flexcore-research) → for each python dir: `pip install -r requirements*.txt`
(pins UNPINNED-REVERIFY / DECISION-POINT — resolve first) → `pytest tests/`
→ CI green run → the lab runs that fill the EMPTY evaluation cells
(P8.09/12/15/17/21/23/26/39) per the filling commands in each record.

**Blockers (human gate inputs):** Stages 1–8 gates NOT HELD; P8.06 frozen
baseline-RESULTS artifact nonexistent (O-1); §19.4 thresholds not
pre-registered (O-2); drill run UNRUN (O-6); advisory lookups
NOT-EXECUTED (O-7); CI UNRUN (O-8); ANDES legal confirmation G-4 (O-13).

**Decisions still needed:** torch/PyG and RL-framework DECISION-POINTs
(O-12, recommendations only); `research.status.read` authz action (O-9);
production SHA-256 verifier crate selection (P8.32 DEFAULT/RECOMMENDATION
recorded); D5/D6 remain OPEN from earlier phases.

**Assumptions (labelled):** ANDES 2.0.0 API surface; HELICS 3.7.0 /
pyhelics; pandapower 3.5.6; framework behavior claims all UNVERIFIED.

**Known risks:** all new code uncompiled/unexecuted — first build may
surface integration defects (one Eq-over-f64 derive was caught and fixed
in the coordinator pass); security F-1 (unguarded torch.load) blocks the
with-framework GNN activation (O-5); proto extensions pending (O-11);
no `research_controller` HELICS orchestrator role (O-10). Full register:
`30_retrospective_exit_gate.md` §4 (O-1..O-15).
