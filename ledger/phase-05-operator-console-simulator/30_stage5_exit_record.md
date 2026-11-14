# P5.30 — Stage 5 exit-gate record and handover to Stage 6 (§23 Stage 5, §24 steps 9–10)

Date: 2026-11-12. Status: **all 33 subtasks implemented (P5.01–P5.33);
nothing installed, nothing built, nothing run** — the §23 Stage 5 gate is
**NOT HELD** (prepared, not demonstrated). The demonstration (step 1), the
two reproducibility proof runs (P5.19), the benchmark figures (P5.23), and
the operator-run-itself check are the human's gate inputs. Everything needed
to hold the gate is written and indexed below.

## 1. Demonstration record — PREPARED, UNRUN (steps 1–2, 6)

The demonstration procedure is fully specified; NO outcome, figure, or
sign-off exists in this record:

1. **Run** — operator executes (from repo root):
   `cargo run -p flexcore-sim-runner -- scenario-run --scenario scenarios/library/lib-02.json --backend scripted`
   then with `--backend worker_process` after `pip install -r apps/worker/python/requirements.txt`
   (pandapower 3.5.6 — NOT installed in this loop). The console view:
   `cd console && npm install && npm run dev` against the control plane
   (`cargo run -p flexcore-control-plane`).
2. **Inspect (step 9)** — every workspace per `29_operator_guide.md` §3;
   the required rejected-command inspection with failing constraint + evidence
   link is the Dispatch workspace's rejection view (P5.06) fed by the
   lib-01/lib-06 records' pre-registered rejection expectations.
3. **Replay (step 10)** — `cargo run -p flexcore-sim-runner -- replay-run --record <run.json> --double-run`
   per `18_20_replay_library.md`; verdict per the pre-registered tolerance
   table (`18_20_replay_library.md` §tolerances).
4. **To record at demonstration time**: scenario ID+version, the operator's
   name (the operator runs it, not the presenting developer — §23 risk row),
   decisions inspected, reproducibility verdict, benchmark figures from
   `23_benchmark_report.md` (EMPTY here), defects found.
5. **Gate sentence**: "an operator can run a full simulated VPP scenario and
   inspect every scheduling and dispatch decision" — **demonstrated, not
   asserted: NOT YET DEMONSTRATED**. Sign-off: NOT SIGNED.

## 2. Handover to Stage 6 (edge/protocol integration) — inputs

| Input | Location | Source |
|---|---|---|
| Console contracts (consumed REST surface) | `apps/control-plane/src/{routes.rs,routes_phase2–4.rs,routes_live.rs,routes_alarms.rs}` + OpenAPI; `console/src/api/*` mirrors | P5.01/25/31 |
| Simulator integration points (adapter inventory) | `14_15_adapters.md` — asset sim REAL, grid worker REAL-but-UNVERIFIED-API, gates seam | P5.14/15 |
| HELICS federation definition | `apps/worker/python/helics_federates/` + `apps/sim-runner/src/helics_orchestration.rs`; three-federate topology in `16_helics_cosim.md` | P5.16 |
| Simulator-track scoping (what Stage 6 inherits) | `17_scoping_decision.md` — comm federate = Stage 6's leg | P5.17 |
| Scenario/replay/library + fault hooks | `apps/sim-runner/src/{replay_record,fault_injection}.rs`, `scenarios/library/` | P5.18–P5.22 |
| Live-update contract | `routes_live.rs` SSE + `console/src/live/ordering.ts` rules | P5.25/33 |
| Alarm model | `crates/flexcore-contracts/src/alarm.rs` (ALARM-MODEL-0.1.0, derived-on-read) | P5.31 |
| Dispatch boundary for edge adapters | `32_handover_phase5.md` §2 — **simulation-only; real device write authority is v1.0.0; edge adapters in Stage 6 are read-only/test-lab by default (release ladder)** | P4.32/P5.13 |

**Known limitations Stage 6 must carry**: unbalanced three-phase from v0.4.0
(`34_unbalanced_deferral.md`); real protocol adapters not built (Stage 6 is
where they land); HIL deferred to Stage 9 (`17_scoping_decision.md`);
operator-approved control is v1.0.0, not now; helics==3.7.0 API usage
UNVERIFIED; all suites UNRUN.

## 3. §22.3 release-gate status for v0.3.0 — NOT MET (prepared)

v0.3.0 = "Validation, feasibility, command lifecycle, approvals, audit —
simulation and shadow". Prepared: the P4.27 evidence pack (70 tests), the
P5.19 reproducibility checker, the transition matrix, the safety-invariant
matrix, this phase's console + demonstration procedure. Missing (all human
gate inputs): the build/test pass across all 16 workspace members + console;
the P4.27 two-clean-runs; reviewer sign-offs; the demonstration (§1); the
P5.23/P5.24 measured figures; the dependency/secret scans.

## 4. Verification tools (step 5) — recorded output

`python tools/verify_docs.py` → OK: headings, section references, fences,
tables, references, TOC. `python tools/validate_agent.py` → OK: skills,
roles, and README index are valid. (Run at close, this date; exit 0 both.)

## 5. CHECKLIST — fully evidenced

G5.1–G5.33 ticked with "**implemented, awaiting human build/test**" and the
decision-record pointer per item; G5.19/G5.30 (the two THE-GATE items) carry
the explicit UNRUN/NOT-DEMONSTRATED status. No item is checked as
"tested"/"verified"/"passed" anywhere in this phase.
