# P8.01 — Phase readiness check: Phases 1–7 outputs and the P7.31 handover

Date: 2026-11-13. Subtask: P8.01 (`ledger_phase_8.md` §P8.01).
Owner role: Research lead. Report trace: §23 Stages 1–7; §19.3; §5.8;
AGENTS.md §6–§7. Inputs: Phase 1–7 ledgers and exit-gate records;
`ledger/phase-07-plugin-sdk/31_phase8_handover.md` (P7.31 — THE handover
input); `ledger/phase-06-edge-protocol/30_retrospective_release_gate.md`
(open items register carried forward); repo at the Phase 7 state.

## 1. Honest status — what was verified and how

| Check | Method actually used | Result |
|---|---|---|
| Documentation / harness structure | `python tools/verify_docs.py`, `python tools/validate_agent.py` (run 2026-11-13, output quoted in §8) | Both OK, exit 0 |
| Workspace dependency rule | `python -I tools/check_dependency_rule.py` (run 2026-11-13, output quoted in §8) | OK — dependency rule holds for 19 workspace members |
| Workspace test suite and the §19.3 benchmark cases (P8.01 step 2) | **NOT RUN — banned in this loop** (ENTRY_NOTE.md) | **UNRUN** — the deterministic baseline's benchmark numbers DO NOT EXIST yet; P8.01 step 2 is not discharged by this record (G-1) |
| Research profiles, worker template, proto contract, sim-runner/seams existence | Source and file inspection (files listed in §4 exist verbatim at the named paths, 2026-11-13) | Present as written artifacts — **nothing verified by execution** |
| P7.31 handover consumption | Read and restated (§3, §5) | Consumed; the P7.31 note's own status is "WRITTEN — awaiting the Phase 8 lead's confirmation"; this record is the Phase 8 lead's reading of it, not the human confirmation |

This record invents no evidence. The workspace has no committed
`Cargo.lock` and has never been built or tested (Phase 6/7 standing
caveat, carried from `01_readiness_check.md` of Phase 7); every artifact
below is "written, awaiting the human build/test pass".

## 2. Stages 1–7 outputs inventory with their TRUE gate status

All seven stage gates are **NOT HELD**. Each phase recorded the exception
honestly at its own close; this table restates those records — it does
not upgrade any of them.

| Stage | Phase | Outputs the platform now holds | Gate status (record, not claim) |
|---|---|---|---|
| 1 | Phase 1 — Rust foundation | Cargo workspace (now 19 members incl. `flexcore-kernel`, `apps/plugin-host`), core crates, REST API skeleton, Postgres/SQLx + NATS adapters, dependency-rule gate | **NOT HELD** — P1.26 demonstration **NOT RUN** (`ledger/phase-01-rust-foundation/26_exit_gate_record.md`); P1.31 **NOT HELD — BLOCKER RECORDED** (`31_exit_gate_review.md`) |
| 2 | Phase 2 — asset & telemetry | Canonical telemetry model (`NormalizedTelemetry`, §10.4 quality vocabulary), `TelemetrySource` port + contract suite, registry use cases, ingest/read stores | **NOT HELD** — P2.26 **NOT RUN** (`ledger/phase-02-asset-telemetry/26_exit_gate_record.md`); P2.29 **NOT HELD — BLOCKER RECORDED** (`29_exit_gate_review.md`) |
| 3 | Phase 3 — flexibility & scheduling | Flexibility envelopes, rolling-horizon scheduling, `Optimizer` port + `HighsTier1Provider` (LP/QP/MIP) + `ClarabelTier2Provider` (convex conic), Tier 3/4 boundary guard | **NOT HELD** — Stage 3 review **NOT HELD** (`ledger/phase-03-flexibility-scheduling/30_handover_phase4.md`) |
| 4 | Phase 4 — grid feasibility & dispatch | `GridModelProvider` port + `PandapowerGridProvider`, verdict/limits/fail-closed, SOCP boundary, command lifecycle (14 states / frozen edges), dispatch security | **NOT HELD** — Stage 4 gate **NOT HELD** (prepared) (`ledger/phase-04-grid-feasibility-dispatch/32_handover_phase5.md` §4) |
| 5 | Phase 5 — console & simulator | Operator console (React/TS), `flexcore-simulator`, deterministic replay, `apps/sim-runner` (scenario/replay/benchmark/fault-injection), reproducibility checker (`apps/sim-runner/src/reproducibility.rs`), scenario library, HELICS federate scaffold, benchmark templates (EMPTY measured tables) | **NOT HELD** — Stage 5 gate **NOT HELD** (prepared, not demonstrated) (`ledger/phase-05-operator-console-simulator/30_stage5_exit_record.md`; P5.19 proof runs UNRUN per `18_20_replay_library.md` §2) |
| 6 | Phase 6 — edge & protocol | `crates/edge-agent`, `EmsAdapter` + ACL, OpenEMS REST adapter (mapping profile v1), store-and-forward buffer, outbound mTLS, enrollment, command path **disabled by default** behind the P6.14 4-condition gate, conformance harness, `deny.toml`, CI gates | **NOT HELD** — lab run **UNRUN** (`ledger/phase-06-edge-protocol/24_exit_gate_lab_run_record.md`); v0.4.0 position recorded **prepared, not demonstrated** (`30_retrospective_release_gate.md` §3) |
| 7 | Phase 7 — plugin SDK | Plugin manifest validator, resolver, lifecycle machine, Class A registry, three WIT worlds + toolchain pins, WASM host (`apps/plugin-host`) with capability broker/enforcement/health/audit-chain, five composition profiles (incl. `research-gnn` / `research-marl` with no-dispatch ceilings), Python gRPC worker template (`worker_api.proto` + `worker_template/`), conformance suite, P7.31 handover | **NOT HELD** — witnessed swap/untrusted-plugin run **UNRUN** (`ledger/phase-07-plugin-sdk/28_exit_gate_run_record.md`); v0.5.0 position recorded **prepared** (`30_retrospective_release_gate.md` §3; P7.31 §9) |

### Gate-exception statement (same pattern as the Phase 7 readiness check)

Phase 8 starts on the recorded exception: **Stages 1–7 gates are all NOT
HELD**, each on the exception recorded in its own exit-gate /
retrospective record (§2). Nothing in this readiness check converts any
NOT HELD into a pass. Every build, test, benchmark run, witnessed run,
scan, and sign-off named in the source records remains a human gate
input.

## 3. What Phase 8 needs that exists (verified in files, 2026-11-13)

| Phase 8 input | Exists? | Evidence |
|---|---|---|
| P7.31 seam table (the handover input) | YES | `ledger/phase-07-plugin-sdk/31_phase8_handover.md` — the research-profile restatement (§2), the reusable worker contracts (§3), the proposal-only enforcement points (§4), the Phase 8 seam table (§5), the non-deliverables (§6), the lift-ready table (§8) |
| Worker template + proto contract | YES — WRITTEN-UNLINTED | `python/workers/contracts/worker_api.proto` (`flexcore.worker.v1`; §9.2 `ResultProvenance`, `DeterminismMetadata`, closed `WorkerErrorCode`), `python/workers/worker_template/` (config/errors/health/handler/server/echo). Status per P7.31 §3/§7: `buf lint` never run; buf-module registration is the recorded activation step (G-2) |
| Research profiles with withheld ceilings | YES | `plugins/profiles/research-gnn.profile.json`, `research-marl.profile.json` — `control-authority: simulation-shadow`; ceiling `telemetry.read` + `asset.read` only; `schedule.propose` and every `dispatch.*` id **WITHHELD** (P7.31 §2). Verified as no-dispatch by reading the profiles and the P7.31 restatement — not by executing the enforcement gate |
| Kernel/host seams | YES (written, UNBUILT) | Kernel manifest/lifecycle (`crates/flexcore-kernel/src/{manifest,lifecycle}.rs`), broker/enforcement/health/audit-chain (`apps/plugin-host/src/{broker,enforcement,health,audit_chain}.rs`), Optimizer (`crates/flexcore-optimization-api/src/port.rs`), GridModelProvider (`crates/flexcore-grid-api/src/port.rs`), telemetry WIT world — all grep-verified by P7.01 §4 and restated in P7.31 §5 |
| Benchmark infra (Phase 5, `apps/sim-runner`) | YES — written, UNRUN | `apps/sim-runner/src/{scenario_run,replay,replay_record,reproducibility,benchmark,fault_injection,metrics,compare,helics_orchestration}.rs`; the P3.27 harness rules (no-targets rule, `not_measured` never rendered as 0) in `benchmarks/README.md` |
| Scenario library (§19.3 cases) | YES — synthetic, UNRUN | `scenarios/library/` (lib-01…lib-08 incl. stress cases: worker timeout, non-convergence, PV ramp, asset dropout, reverse power flow) + `scenarios/synthetic-portfolio/`; per-case IEEE exclusion records and pre-registered UNRUN expected outcomes (`scenarios/library/INDEX.md`) |
| Reproducibility checker (P5.19) | YES — written, UNRUN | `apps/sim-runner/src/reproducibility.rs` + `apps/sim-runner/tests/reproducibility_p5_19.rs`; the double-run proof and negative control are **UNRUN** (`ledger/phase-05-operator-console-simulator/18_20_replay_library.md` §2) — the rerun discipline research results will key on is itself unproven (G-8) |
| HELICS scaffold + carried pin | YES — written, NEVER installed | `apps/worker/python/helics_federates/` (federates, broker, config example, `requirements-helics.txt` with `helics==3.7.0`); Phase 5 RESEARCH.md §1 records HELICS 3.7.0 (25 Aug 2026); the pyhelics binding match remains that phase's open task 3 (G-6) |
| Solver pins for baselines | YES — recorded, UNRUN | `clarabel = "=0.11.1"` in the workspace `Cargo.toml`; HiGHS tier-1 provider in `crates/flexcore-optimization-api`; solver-class declarations per §7.4 discipline |
| Phase 6 open-items register | YES | `ledger/phase-06-edge-protocol/30_retrospective_release_gate.md` §2 — carried into §6 below; the edge command path stays **disabled by default** and research uses simulated/site-lab data only |

## 4. What is PREPARED-NOT-PROVEN (carried into Phase 8 as such)

1. **Every workspace crate is written, UNBUILT, UNRUN.** No compiled
   plugin host exists; no seam is a verified-by-execution contract. The
   first human `cargo build` / `cargo test --workspace` is the proof
   point (Phase 7 O-1).
2. **The deterministic baseline's benchmark numbers DO NOT EXIST.**
   P8.01 step 2 (workspace test suite + §19.3 benchmark cases) is UNRUN;
   the P3.27/P5 benchmark tables carry `not_measured`/EMPTY cells, and
   the P5.19 reproducibility proof runs are UNRUN. The "strong
   deterministic baseline" the Stage 8 exit gate compares research
   modules against is currently an empty template — running it is the
   blocking proof input for P8.06 (G-1).
3. **No Python research environment has ever been installed.**
   `grpcio`/`protobuf`/`pytest` pins are UNPINNED-REVERIFY with
   deliberately un-installable requirement lines (`python/workers/
   requirements.txt` honesty header); pandapower 3.5.6 is RECORDED,
   NEVER INSTALLED (`apps/worker/python/requirements.txt`); ANDES,
   HELICS, torch/PyG/JAX, Ray/RLlib, Gymnasium are recorded statically
   only (`RESEARCH.md`; `03_research_license_audit.md`). The worker
   template has never served a request (P8.01 step 3's "worker template
   runs" is UNDISCHARGED — G-3).
4. **The `worker_process` grid backend is UNVERIFIED pandapower API** —
   lib-04/lib-05 legs were written, never executed (`scenarios/library/
   INDEX.md`). Research grid-feasibility comparisons must not assume
   that leg works.
5. **`worker_api.proto` is WRITTEN-UNLINTED and unregistered** in the
   `proto/` buf module (P7.31 §3/§7). P8.04 extends the contract only
   after registration + first `buf lint`/`buf breaking` (G-2).
6. **No research provider of any kind exists** — no GNN, MARL, ADMM/DMPC,
   ANDES, or HELICS code; no `research.*` manifest; no benchmark suite;
   no GNN/MARL result contract. A generic `Optimizer` world is NOT a
   GNN contract (P7.31 §1/§6). The research profiles are reviewed
   shells whose research-worker/benchmark/shadow-logger selections are
   commented-out placeholders — no such manifest exists.
7. **CI has no green run on record** (P6.27/P7.27 written, UNRUN);
   `cargo deny` has never run; no `Cargo.lock` exists (`deny.toml`
   header) — every license/advisory verdict is static until the first
   human run (`03_research_license_audit.md` §1).
8. **D1–D6 are OPEN** (`.agent/DECISIONS.md`). D3 (network class) and
   D4 (license audit) directly gate this phase's experiments; the
   0.x balanced-radial default is a RECOMMENDATION, not a decision.
9. **Phase 6 carried items must not be assumed** (P6.30 §2 / P7.30
   O-16): uplink wire format OPEN (`NoBatchPayloadEncoder` REFUSES),
   TLS on the OpenEMS REST hop OPEN, OpenEMS API shape A1–A8 UNVERIFIED.
   Research runs on simulated data and does not touch the OT boundary,
   so these do not block the experiments — but no research claim may
   cite them as working.

## 5. Gap list (P8.01 step 4)

| ID | Gap | Class | Owner | Blocks what |
|---|---|---|---|---|
| G-1 | Workspace build/test pass + §19.3 benchmark-case run UNRUN (P8.01 step 2); baseline benchmark results with versions/seeds do not exist; P5.19 reproducibility proofs UNRUN | Verification gap — the phase's single blocking proof input | Human build/test pass (first `cargo test --workspace` + benchmark run) | **Blocking rule applied (ledger P8.01 step 4):** research-provider *evaluation* against the baseline (P8.06 freeze, then P8.09/P8.12/P8.15 comparisons) is blocked until real baseline numbers exist. Design work that consumes contracts, not numbers (P8.02/P8.03/P8.04) proceeds on the recorded exception |
| G-2 | `worker_api.proto` unlinted, unregistered in the `proto/` buf module | Recorded seam (P7.31 §7; P7.30 O-7) | Coordinator / activation wave | P8.04 must not extend the contract before registration + first lint |
| G-3 | grpcio/protobuf/pytest UNPINNED-REVERIFY; worker template never executed | Dependency-verification gap (P7.30 O-8) | Activation wave after version verification (procedure in `python/workers/requirements.txt` header) | Any research worker execution (P8.05 harness runs, P8.11/P8.14 workers) |
| G-4 | ANDES GPLv3 isolation design recorded but NOT legal-confirmed | D4 gate | Legal + research lead (`03_research_license_audit.md` §3) | Any ANDES dependence (P8.16) |
| G-5 | PyTorch/PyG version skew unresolved (R8-C3); no verified-compatible pair pinned | Recorded DECISION-POINT (`03_research_license_audit.md` §4) | Research engineer (ML) + legal | GNN stack pin (P8.10 step 4) |
| G-6 | HELICS 3.7.0 pin carried; the matching pyhelics binding version unconfirmed (Phase 5 RESEARCH open task 3) | Verification gap | Phase 8 co-sim owner (P8.18) | HELICS federation run (P8.18) |
| G-7 | Ray/RLlib current version unverified from a primary source (RESEARCH.md §7 task 1); RLlib API stack (new vs old) unselected (R8-C4) | Verification gap | Research engineer (RL) (P8.13) | MARL framework pin (P8.13 step 4) |
| G-8 | D3 (network class) OPEN — each experiment must state the class it covers; the default is a recommendation, not a decision | Open decision gate (`.agent/DECISIONS.md`) | Power systems (D3 owner); recorded per experiment in P8.02 | Any experiment whose results are read outside its declared class |
| G-9 | Phase 6 carried opens (uplink wire format, OpenEMS TLS, A1–A8) | Prior-phase open items (P6.30 O-4/O-5) | Phase 6 owners | Not blocking research (simulated data only); must not be assumed anywhere |

## 6. Verdict

**PREPARED, not proven.** Every structural input Phase 8 needs exists as
a written artifact: the P7.31 seam table, the worker template and proto
contract, the two no-dispatch research profiles, the kernel/host seams,
the sim-runner benchmark/replay/reproducibility infrastructure, the
synthetic scenario library, the HELICS scaffold with the carried 3.7.0
pin, and the recorded solver pins. Nothing has been executed: all seven
stage gates are NOT HELD on recorded exceptions, the baseline benchmark
numbers do not exist, no research environment is installed, and no
research module of any kind has been written. Phase 8 builds on prepared
material and must keep saying so — in particular, no research result may
ever be compared against a baseline that was never run (the ledger's
P8.01 risk, restated as G-1's blocking rule).

## 7. Verification method

Re-run the three quoted commands (§8) and re-run the benchmark cases
from the recorded inputs once the human build/test pass exists; compare
the recorded baseline numbers (when P8.06 freezes them) against the
re-run.

## 8. Verification-tool output (quoted, run 2026-11-13)

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
OK — dependency rule holds for 19 workspace members.
(exit 0)
```

These tools check documentation and structure only. They do not compile
or test Rust and do not run any benchmark; the build/test pass and the
§19.3 baseline run remain the human's (G-1).
