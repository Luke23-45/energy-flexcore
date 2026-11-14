# P8.24 — Failure-mode and robustness drill (research modules): the drill PLAN + EMPTY run record

Date: 2026-11-14. Subtask: P8.24 (`ledger_phase_8.md` §P8.24). Owner role:
research lead. Report trace: §19.2(6) fault set; §13.3 (delayed/lost
messages, uncertain state estimates). Inputs: P8.08
(`08_admm_operations.md` — the failure-mode matrix this drill extends to
the whole research layer), P8.11 (`11_gnn_inference_worker.md`), P8.14
(`14_marl_worker.md`), and the ACTUAL code read on 2026-11-14:
`crates/flexcore-research/src/{dmpc,admm,validation}.rs`,
`apps/worker/python/{marl_worker,gnn_worker,andes_worker,
portfolio_analytics}/**`. Structure mirrors the Phase 7 precedent
`ledger/phase-07-plugin-sdk/30_failure_drill.md`.

## 0. Run-status honesty — READ FIRST

**The drill has NOT been run.** Nothing was executed in this loop (the
loop bans; ENTRY_NOTE.md): no worker process was started, no fault was
injected, no advisory was queried, no pytest/cargo run happened. **Every
measured cell in the §3 run record is deliberately EMPTY (UNRUN — human
gate input).** The "expected fail-closed behaviour" column in §2 is a
DESIGN statement derived FROM READING the named code (every cell cites
the file and the named error/outcome that was verified to exist by
reading on 2026-11-14) — it is NOT evidence that the drill ran, and it
is not a claim that the behaviour works. The independent witness
re-running TWO cells (the ledger's verification method) is a human-gate
step: witness identity and date are EMPTY fields in §3, filled only by
the witness.

## 1. What the drill covers

One combined drill over the §19.2(6) fault set as it manifests on the
Phase 8 research layer: the ADMM/DMPC coordinator
(`crates/flexcore-research/src/dmpc.rs` + `admm.rs`), the independent
validation harness (`crates/flexcore-research/src/validation.rs`), and
the four Python research workers (`marl_worker/`, `gnn_worker/`,
`andes_worker/`, `portfolio_analytics/`). The ledger P8.24 step-1 fault
list is mapped to modules; each fault names the code seam it exercises.

**A recorded coverage note (honest, before the table):** the in-crate
ADMM worker port is a synchronous call-return seam
(`dmpc.rs::AdmmLocalWorker` — one `solve_local` call, one
`Result<LocalSolveReply, LocalSolveError>`; `dmpc.rs` ~295–310). At THAT
seam a reordered message is not representable (there is no queue), and a
duplicated message is not representable at the port either — it is a
TRANSPORT-layer event. The scaffolding models both shapes over injected
callables (`apps/worker/python/drill/fault_injection.py`:
`replay_last`, `ReorderInjector`) so the cells are runnable at the
human gate against the composition root's real transport, and the
in-crate expectations below are stated only for the shapes the port can
express. Re-ordering INSIDE the coordinator is additionally bounded by
construction: the x-update walks participants in fixed sorted-id order
(`admm.rs` header, determinism rule), so the coordinator never
reorders.

## 2. The fault/module PLAN — expected fail-closed behaviour, derived from the code read 2026-11-14

Modules: **M-A** ADMM/DMPC coordinator (`dmpc.rs`/`admm.rs`); **M-V**
independent validation harness (`validation.rs`); **M-G** GNN worker
(`gnn_inference_worker.py`); **M-M** MARL worker (`marl_worker.py` +
`safety_filter.py` + `shadow_log.py`); **M-N** ANDES worker
(`andes_dynamic_worker.py` + `evaluation_harness.py`); **M-P** portfolio
analytics (`analytics.py`). "n/a" means the module has no such seam and
the cell is recorded as not-applicable at module level (the transport
seam is the composition root's).

### F1 — dropped / duplicated messages

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | A dropped reply surfaces as `Err(LocalSolveError)` from the port → one staleness unit, the last accepted value reused, the reuse RECORDED as a `StaleEvent` (`dmpc.rs` x-update match arms, ~842–883); beyond `max_stale_iterations` → `ABANDONED_PARTICIPANT_UNRESPONSIVE` with participant and iteration named (`dmpc.rs::RunStatus::name`, ~523) and the fallback recorded. A duplicated delivery (modelled by `replay_last`) reaches the coordinator as a normal fresh reply — the in-crate coordinator does NOT check the `LocalSolveRequest.iteration` field on replies (`dmpc.rs` ~252–260; the field exists, no in-crate consumer) — recorded as observation O-2 (§5), never silently patched. |
| M-V | A re-presented (duplicated) proposal byte-identical to a validated one re-runs stages 1–6 (`validation.rs::validate`, ~761+): within its window with a fresh snapshot it re-validates — the harness has NO idempotency/dedup gate (no seen-id set exists in stages 1–6). Recorded as finding D-1 (§5) with the upstream owner named; the drill cell records the observed re-validation, never invents a dedup refusal. |
| M-G | One JSON document in, one out; the worker answers each request exactly once (`gnn_inference_worker.py::main`, 1016–1051) — a dropped request is caller-visible silence, a duplicated request gets two independent fail-closed answers; the worker cannot detect duplication (recorded limitation; idempotency is the caller's seam). |
| M-M | Same transport shape (`marl_worker.py::main`, 861–897); a duplicated `filter_probe` request yields two independent filter verdicts, each logged as its own sequence-numbered shadow-log entry (`shadow_log.build_log_entry` requires sequential numbers — `ShadowLogError` on gaps, ~237–273) — the log makes the duplicate VISIBLE, not deduped. |
| M-N | Same transport shape (`andes_dynamic_worker.py::main`, 665–696); two identical study requests produce two independent in-band results (a duplicated study is a duplicated cost, never a correctness hole — results are proposal-only diagnostics). |
| M-P | n/a — pure functions over injected records; no message seam. |

### F2 — reordered messages

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | NOT representable at the call-return port (§1 note); the coordinator consumes replies in fixed sorted-id order, so participant order never varies. The transport-level reorder cell runs at the composition root over the gRPC seam (the Phase 7 P7.18 supervision domain) — the scaffolding's `ReorderInjector` models the delivery shape for that run. Recorded as coverage limitation C-1 (§5), not as in-crate behaviour. |
| M-G/M-M/M-N | n/a at module level — the JSON-document transport is one-request-one-reply; reordering exists only at a queueing transport the workers do not use (C-1). |
| M-V | n/a — proposals arrive through `proposal_for_first_step` in fixed interval order (`dmpc.rs` ~1227; `plan_horizon` walks intervals in order, `08_admm_operations.md` §3). |
| M-P | n/a. |

### F3 — clock skew

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-V | The validity window runs against the INJECTED clock (`validation.rs` Stage 4, ~620–634): a skewed clock shifts WHERE the refusals fire, never WHETHER — an expired proposal is refused `PROPOSAL_EXPIRED` and a not-yet-open one `PROPOSAL_NOT_YET_VALID`, regardless of any wall clock (no wall clock exists in the harness). Drill cell: sweep the injected clock across the window edges and record the refusal boundary; the scaffolding's `skewed_clock` provides the skewed clock. |
| M-A | n/a in-crate — no wall clock anywhere (determinism; `08_admm_operations.md` §5: runtime is deliberately NOT computed); timestamps are composition-root records. |
| M-G | The DECLARED `timeout_seconds` is validated against the recorded maximum `MAX_INFERENCE_TIMEOUT_S = 60.0` (gnn ~151, 678–689; over-max and non-positive refused `WorkerRequestError`, exit 2). The mechanism that ENFORCES the timeout at runtime is a recorded UNVERIFIED item (`11_gnn_inference_worker.md` §8 item 5) — a clock-skew-vs-declared-timeout gap cannot be bounded in-crate today; the cell records that honestly. |
| M-M | n/a — no clock input; `execution_ms` is caller-recorded. |
| M-N | Simulation/event times are declared and validated finite/positive (`andes_dynamic_worker.py` 284–299, 345–350); a skewed wall clock has no in-worker surface (the ANDES-internal timebase is model scope, X-5 boundary). |
| M-P | n/a — intervals are declared per record (`interval_hours` validated > 0, `analytics.py` ~172–180). |

### F4 — stale telemetry / uncertain state estimates (§13.3)

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | The honest stale flag `LocalSolveReply.stale` (`dmpc.rs` ~268–276) → the flagged reply is DISCARDED (never silently accepted as fresh), the last accepted value reused, and every reuse recorded as a `StaleEvent` with `used_stale_values = true` on the result (`dmpc.rs` ~842–860); beyond the bound → `ABANDONED_PARTICIPANT_UNRESPONSIVE`. Hand-pinned in the crate's own suite: stale-forever with bound 1 → abandon at iteration 2 (`dmpc.rs` ~1933–1936); stale once then healthy → converged with the flag visible (~08 matrix row 7). |
| M-V | A proposal computed against stale conditions is refused at Stage 5 snapshot freshness: `SNAPSHOT_SUPERSEDED` (superseded by a named snapshot) or `SNAPSHOT_UNKNOWN` — "unknown is never fresh" (`validation.rs` ~637–659). |
| M-G | Non-finite / out-of-range stale garbage refused BEFORE any framework import: features must be finite, SOC in [0,1], voltage > 0, status in {0,1} (`validate_features`, gnn ~406–457). A PLAUSIBLE-but-stale value is NOT detectable by the worker — the request contract carries NO telemetry quality-state field (the platform's GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE states travel platform-side) — recorded as gap D-2 (§5). An out-of-scope topology is flagged in-band with NULL outputs (`evaluate_topology_scope`, gnn ~460–489). |
| M-M | Malformed action → `FilterInputError` normalized into `WorkerRequestError` (marl ~397–400; `safety_filter.py::validate_action`); declared limits are in-request (the manifest-in-request stopgap, `14_marl_worker.md` §8 item 4) — a false declaration is visible because every filter decision is RECORDED with before/after and the allowed interval (`safety_filter.py` activations, ~326–523), never silently trusted. |
| M-N | Metric series: non-finite values, non-increasing times, length mismatch, one-sample windows all refused with `MetricInputError` (`evaluation_harness.py` ~82–110, 118–130) — a stale/garbage trace cannot silently produce a nadir/settling figure. |
| M-P | Every event record must carry `quality_state` EXACTLY one of the §10.2 telemetry quality states, carried on the output, never stripped (`validate_flexibility_event`, `analytics.py` ~202–208); unknown/malformed records refused `EVENT_INVALID`. |

### F5 — unavailable assets / partial fleet response

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | A wiring gap is refused BEFORE any run: `WORKER_COVERAGE_MISSING` at construction (`dmpc.rs` ~674, coverage validated in `AdmmCoordination::new`) — it can never masquerade as a mid-run drop. A partially responsive fleet continues with recorded reuse until the staleness bound; beyond it the run is abandoned WITH the participant and iteration named (`dmpc.rs` ~837–883; hand-pinned: worker never answers, bound 0 → abandon at iteration 1, ~1897–1913). |
| M-M | A partial action (some entries feasible, some not): per-entry outcomes aggregate to the closed vocabulary; an EMPTY feasible interval anywhere → the aggregate `infeasible_no_safe_projection` with the fallback attempted and PROVABLY not viable, recorded on the log entry and counted in the summary (`safety_filter.py` ~468–506; `shadow_log.log_summary` infeasibility count) — never an unfiltered pass-through (X-2 posture). |
| M-G | n/a (single-graph request; a shape mismatch is a `WorkerRequestError` refusal, not a partial answer). |
| M-N | An unavailable/missing case or output surface: `andes.load` failure or an absent `PFlow`/`EIG` surface → IN-BAND `termination_status: "failed"` with diagnostics (`andes_dynamic_worker.py::run_study`, ~523–560, 655) — never a crash, never a fabricated figure. |
| M-V | n/a. |
| M-P | An empty record list is refused (`AnalyticsInputError`, `aggregate_delivery` ~283–300) — no aggregate is computed over nothing. |

### F6 — solver timeout

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | Budget exhausted → `NON_CONVERGED_BUDGET_EXHAUSTED` with the final residual pair recorded, NEVER a result (`08_admm_operations.md` §2 step 1; `dmpc.rs::RunStatus` ~520); the fallback is invoked and its outcome recorded (`FallbackOutcome`/`fallback_error`); a refused fallback (`FALLBACK_CLASS_NOT_SUPPORTED`, `dmpc.rs` ~394) leaves NO schedule. |
| M-G | Declared timeout above the recorded maximum refused (`WorkerRequestError`, gnn ~683–689); runtime enforcement UNVERIFIED (gap D-3, §5). |
| M-M | n/a (no solver; training is guarded and unreachable). |
| M-N | No in-worker timeout mechanism exists (recorded gap D-4, §5); a hung ANDES run is the composition root's supervision concern (Phase 7 P7.18 precedent: timeouts, bounded retries, crash → FAILED). |
| M-V | A hung/failed feasibility gate is a REFUSAL, never a pass: `PHYSICAL_REVALIDATION_SEAM_FAILED` — "fail-closed, never a pass" is the recorded seam rule (`validation.rs` ~43, 314–326, 788–795; pinned by `gate_seam_error_is_fail_closed_never_a_pass`, ~1374–1386). |
| M-P | n/a. |

### F7 — worker/plugin crash (and the harness-survival cell — the ledger's named risk)

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | An unresponsive/failed worker is a staleness unit per iteration (above); a REAL process crash is the composition root's supervision seam (P7.18 precedent, `apps/worker/src/supervision.rs` cited in the Phase 7 drill F1) — in-crate the reference worker is panic-free by construction (closed-form local update). Recorded as coverage limitation C-2: the in-crate cells exercise the `Err` path; the process-kill cell runs at the composition root. |
| M-V (THE NAMED RISK: "a worker crash that takes down the validation harness with it") | The harness consumes the gate port's `Result`: ANY `Err` is the named refusal `PHYSICAL_REVALIDATION_SEAM_FAILED` and the proposal is refused — a gate failure cannot crash the harness NOR pass a proposal (`validation.rs` 788–795). HONEST observation O-1 (§5): this holds for the `Err` surface; a PANICKING gate impl would unwind through `validate` — panic containment across the real gate is the composition root's process boundary, not the harness's. The drill cell injects a gate that returns `Err(GateSeamError)` and records the refusal; the panic cell is recorded as C-2's composition-root run. |
| M-G/M-M/M-N | Process isolation (Class C, ADR-004): the worker IS the process; a crash kills only itself and produces NO stdout envelope — the envelope is built fully, scanned by `assert_proposal_only`, and written ONCE at the end (gnn 1035/1048; marl 881/894; andes 684/694), so no partial envelope can exist. Refusals exit 2 with the reason on stderr. |
| M-P | n/a — in-process pure functions; no crash seam at module level. |

### F8 — communication loss between ADMM participants / participant killed mid-iteration

| Module | Expected fail-closed behaviour (code-derived, UNRUN) |
|---|---|
| M-A | A participant killed mid-iteration answers `Err` from then on → staleness units accumulate → `ABANDONED_PARTICIPANT_UNRESPONSIVE` at the bound, participant AND iteration named (`dmpc.rs` match arms ~862–883; hand-pinned rows 5–6 of the `08_admm_operations.md` §4 matrix). The abandoned run's partial `x` is diagnostics-only: `final_schedule()` returns `Option` and there is NO constructor for a schedule from a partial ADMM state (`dmpc.rs` ~616; `08_admm_operations.md` §2 step 3–4) — a dropped participant produces a recorded failure or a fallback, NEVER a silently partial result. |
| M-V | A proposal cannot arise from a lost-participant run: `proposal_for_first_step` (`dmpc.rs` ~1227) packages only the first step of a FINAL schedule (converged ADMM or recorded fallback); an abandoned run yields no proposal artifact. Drill cell: run the abandoned scenario and assert no proposal is produced. |
| M-G/M-M/M-N/M-P | n/a — not ADMM participants. |

## 3. The RUN record — per-fault-per-module table (ALL MEASURED CELLS EMPTY — UNRUN, human gate input)

The ledger's deliverable. The witness re-runs TWO cells of their choice
from this plan alone and reproduces the outcomes (the ledger
verification method) — witness identity and date are human-gate fields.
"Invalid proposal reached validation?" means: did any output of a
faulted module enter the P8.05 harness vocabulary and pass?

| Fault | Module | Injection used | Behaviour observed | Flagged / rejected / fell back? | Invalid proposal reached validation? | Witness (human gate) |
|---|---|---|---|---|---|---|
| F1 dropped/duplicated | M-A | (EMPTY — UNRUN) | | | | (EMPTY) |
| F1 dropped/duplicated | M-V | (EMPTY — UNRUN) | | | | (EMPTY) |
| F1 dropped/duplicated | M-G | (EMPTY — UNRUN) | | | | (EMPTY) |
| F1 dropped/duplicated | M-M | (EMPTY — UNRUN) | | | | (EMPTY) |
| F1 dropped/duplicated | M-N | (EMPTY — UNRUN) | | | | (EMPTY) |
| F2 reordered | M-A (transport, C-1) | (EMPTY — UNRUN) | | | | (EMPTY) |
| F2 reordered | M-G/M-M/M-N (C-1) | (EMPTY — UNRUN) | | | | (EMPTY) |
| F3 clock skew | M-V | (EMPTY — UNRUN) | | | | (EMPTY) |
| F3 clock skew | M-G | (EMPTY — UNRUN) | | | | (EMPTY) |
| F4 stale telemetry | M-A | (EMPTY — UNRUN) | | | | (EMPTY) |
| F4 stale telemetry | M-V | (EMPTY — UNRUN) | | | | (EMPTY) |
| F4 stale telemetry | M-G | (EMPTY — UNRUN) | | | | (EMPTY) |
| F4 stale telemetry | M-M | (EMPTY — UNRUN) | | | | (EMPTY) |
| F4 stale telemetry | M-N | (EMPTY — UNRUN) | | | | (EMPTY) |
| F4 stale telemetry | M-P | (EMPTY — UNRUN) | | | | (EMPTY) |
| F5 partial fleet | M-A | (EMPTY — UNRUN) | | | | (EMPTY) |
| F5 partial fleet | M-M | (EMPTY — UNRUN) | | | | (EMPTY) |
| F5 partial fleet | M-N | (EMPTY — UNRUN) | | | | (EMPTY) |
| F6 solver timeout | M-A | (EMPTY — UNRUN) | | | | (EMPTY) |
| F6 solver timeout | M-V | (EMPTY — UNRUN) | | | | (EMPTY) |
| F6 solver timeout | M-G | (EMPTY — UNRUN) | | | | (EMPTY) |
| F7 worker crash + harness survival | M-V | (EMPTY — UNRUN) | | | | (EMPTY) |
| F7 worker crash | M-G | (EMPTY — UNRUN) | | | | (EMPTY) |
| F7 worker crash | M-M | (EMPTY — UNRUN) | | | | (EMPTY) |
| F7 worker crash | M-N | (EMPTY — UNRUN) | | | | (EMPTY) |
| F8 participant killed mid-iteration | M-A | (EMPTY — UNRUN) | | | | (EMPTY) |
| F8 no-proposal-from-abandoned-run | M-V | (EMPTY — UNRUN) | | | | (EMPTY) |

## 4. The drill scaffolding (the executable spec's tooling)

`apps/worker/python/drill/fault_injection.py` (NEW, pure stdlib, never
run): fault injection over INJECTED callables — closed fault vocabulary
(`none`, `error`, `drop`, `crash_before`, `crash_after`,
`scripted_reply` for the honest stale re-serve, `replay_last` for
duplicated delivery, `ReorderInjector` for out-of-order scripted
delivery), plus `skewed_clock` (fixed ms offset for the M-V window cell)
and `oversized_payload` (exactly one byte over the harness cap
`MAX_PROPOSAL_PAYLOAD_BYTES` = 1048576, verbatim from
`validation.rs`). Every expectation is hand-computed in the in-module
self-tests and mirrored in
`apps/worker/python/drill/tests/test_fault_injection.py`. NOTHING is
executed by this wave; the human gate runs:

```sh
python -I apps/worker/python/drill/fault_injection.py
python -m pytest apps/worker/python/drill/tests/test_fault_injection.py -v
```

and then wires the cells of §3: the M-A cells run against the injected
`AdmmLocalWorker` port per the `08_admm_operations.md` §7 commands
(`cargo test -p flexcore-research` runs the §4 matrix); the worker cells
pipe the hostile request documents through the workers'
`main()` entrypoints; the M-V cells drive `validation.rs` through its
own suite plus the scaffolding's skewed/oversized inputs at the
composition root. Real-process kills (SIGKILL mid-iteration) are
composition-root runs (C-2), never scaffolding runs.

## 5. Honest observations recorded while planning (not silently patched)

1. **O-1 — the harness-survival guarantee is a `Result` guarantee.**
   `validation.rs` converts a gate `Err` into
   `PHYSICAL_REVALIDATION_SEAM_FAILED` (fail-closed, pinned by its own
   test); a PANICKING gate implementation is outside the `Result`
   surface and would unwind through `validate`. The drill records the
   `Err` cell in-crate; the panic cell belongs to the composition
   root's process containment (C-2).
2. **O-2 — the in-crate coordinator does not check
   `LocalSolveRequest.iteration` on replies.** The field exists on the
   request (`dmpc.rs` ~252–260) but no in-crate consumer validates that
   a reply belongs to the iteration asked; a transport that delivers a
   stale-iteration reply would be accepted as fresh (until the residual
   evidence says otherwise). Recorded for the composition root's
   transport checks — no fix invented here.
3. **O-3 — the validation harness has no dedup/idempotency gate.** A
   duplicated valid proposal re-validates (stages 1–6, no seen-id set).
   Dedup for research proposals is recorded as an upstream/Phase 7
   follow-up question (§6), not as a harness change made here.
4. **O-4 — the Python worker request contracts carry no telemetry
   quality-state field** (contrast M-P, which REQUIRES one on every
   event record). Staleness detectable by the workers is limited to
   shape/finiteness/range checks. Recorded as gap D-2 / Phase 7
   follow-up (§6).

## 6. Contract gaps → Phase 7 follow-ups (recorded, never invented fixes)

| # | Gap | Where recorded |
|---|---|---|
| D-1 | No dedup/idempotency gate for re-presented research proposals (O-3) — the SUPERSEDED/command-lifecycle state machine is the platform's existing path; whether research proposals need their own dedup seam is a P8.04/Phase 7 owner question | this record §5.3; not implemented |
| D-2 | No telemetry quality-state field in the GNN/MARL/ANDES worker request contracts (O-4) — an additive `worker_api.proto`/request-contract change for the P8.04 owner, never a parallel contract (X-10) | this record §5.4; `11_gnn_inference_worker.md` §8 |
| D-3 | Runtime timeout enforcement in the GNN worker is UNVERIFIED (`11_gnn_inference_worker.md` §8 item 5) — the drill's F6/M-G cell records the honest gap | carried from the P8.11 record |
| D-4 | No in-worker timeout mechanism for ANDES studies — supervision belongs to the composition root (P7.18 seam); a recorded composition-root obligation, not a worker change | this record §2 F6/M-N |
| C-1 | Reorder cells are not representable at the in-crate call-return port — they run at the transport (composition root) with the scaffolding's `ReorderInjector` | §1, §2 F2 |
| C-2 | Process-kill cells (crash_before/crash_after against REAL worker processes; a panicking gate) are composition-root runs, not in-crate/scaffolding runs | §2 F7, §5.1 |

## 7. Acceptance rule (recorded)

**Every fault/module cell must have a recorded outcome, and ZERO faults
may let an invalid proposal pass validation or crash the control plane**
(ledger P8.24). Until §3 is filled from actual runs, that criterion is
NOT met and is NOT claimed — the empty table is the honest current
state, exactly as the P7.30 precedent recorded. The fix loop (ledger
step 3): any fault that lets an invalid proposal pass or crashes the
control plane is fixed and re-drilled until clean; a fix is a code
change by its owning owner, never a drill-report edit.

## 8. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (must still report OK; no dependency edge added) |
| The drill itself (any cell of §3) | **NOT RUN — banned in this loop; human-gate input; witness fields EMPTY** |
| Scaffolding self-tests | **WRITTEN, NOT RUN** (hand-computed expectations only; run instruction in §4) |
