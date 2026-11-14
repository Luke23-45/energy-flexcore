# P8.16 — The ANDES dynamic-simulation research worker
#         (`apps/worker/python/andes_worker/`)

Status: **WRITTEN, NEVER EXECUTED — UNVERIFIED-API, GPLv3-ISOLATED.** The
Phase 8 loop bans install/build/run: ANDES 2.0.0 (`requirements-andes.txt`,
RESEARCH.md §4: released 11 March 2026) was NEVER installed, this worker
was NEVER run, and no dynamic-study result anywhere in this repository was
produced by it. Every `andes` call in `andes_dynamic_worker.py` follows the
DOCUMENTED ANDES 2.0 API as of the 2026-11-07 research cut-off and is
**UNVERIFIED** until the human build/test pass executes it against the
pinned version (R8-C5: pin 2.0.0 or a VERIFIED later release; the live
docs' development version string is not a version citation).

## What this worker is — and what it is not

The worker runs ANDES time-domain simulation and/or eigenvalue analysis on
a DECLARED case and model scope, and returns the outputs **as data** (one
JSON result envelope) with the metrics of the pure P8.17 evaluation
harness computed on actually extracted traces. It is:

- **GPLv3-isolated** (P8.03 six-point design; gate **G-4 legal
  confirmation OPEN**): ANDES lives only inside this worker process; the
  import is guarded inside `import_andes()` with the NAMED deployment
  requirement; FlexCore consumes ANDES outputs as data, never ANDES-derived
  code; no core/SDK crate ever depends on ANDES and nothing ANDES-derived
  is distributed without legal review.
- **Proposal-only** (ADR-010): the result type carries no authorization
  token and no device command; `assert_proposal_only()` scans every key of
  the result tree and refuses emission of any forbidden key. The worker has
  no field-write path — its only output channel is stdout.
- **Mode-locked to simulation/shadow** (§21.4): a constant, not a setting;
  no request field can change it.
- **Boundary-disciplined** (§19.1): a steady-state power flow is NOT
  transient-stability evidence; the dynamic simulator does NOT replace the
  distribution quasi-static pandapower pipeline; every result carries its
  model scope and the boundary statements verbatim.

The pure-logic core (request validation, result envelope, proposal-only
scan) is STDLIB-ONLY and importable/testable WITHOUT ANDES installed —
which is also the loop's honesty gate: without ANDES, no study result can
ever exist (`run_study` raises the named deployment requirement).

## Files

| File | Content |
|---|---|
| `andes_dynamic_worker.py` | The worker entrypoint: one `flexcore-andes-worker/1` JSON request on stdin → one result envelope on stdout; guarded ANDES import; UNVERIFIED call/assumption table in the header; fail-closed request handling; in-band study failures |
| `evaluation_harness.py` | The P8.17 PURE dynamic-response metrics (nadir, zenith, max ROCOF, damping ratio by log decrement, settling time) on injected time series, with HAND-COMPUTED in-module self-tests (`run_self_tests()`) |
| `requirements-andes.txt` | The recorded ANDES pin (RECORDED, NEVER INSTALLED; D4/G-4 audit input, not a lockfile) |
| `tests/test_evaluation_harness.py` | The harness test suite (mirrors the in-module self-tests; WRITTEN, NEVER RUN) |
| `tests/test_andes_dynamic_worker.py` | The worker's pure-logic test suite: request validation, proposal-only enforcement, the named ANDES import gate, fail-closed main (WRITTEN, NEVER RUN) |

## Transport contract

ONE JSON document on stdin (the `flexcore-andes-worker/1` request: protocol
tag, `study_type` ∈ {`time_domain`, `eigenvalue`, `steady_state_power_flow`},
`case_path`, `case_version`, `models`, `simulation` + `monitored` +
optional `evaluation` for time-domain, optional `event`,
`provenance.input_snapshot_id` + `request_id`), ONE JSON document on
stdout (the result envelope: model scope, outputs, metrics, §9.2-shaped
provenance, boundary statements, proposal-only posture). A malformed
request is answered with NOTHING on stdout + a reason on stderr + exit 2.
A study that fails AT RUN TIME (non-convergence, unreadable case, missing
output surface) is answered IN-BAND (`termination_status: "failed"` +
diagnostics) — a failed study is a result, never a crash, and no figure is
ever fabricated.

## How the human build/test pass runs it (the recorded procedure)

1. **Legal first (gate G-4):** the P8.03 isolation design
   (`ledger/phase-08-advanced-coordination/03_research_license_audit.md`
   §3) requires legal confirmation of the ANDES GPLv3 posture BEFORE any
   dependence. This worker's existence does not grant it.
2. Create an ISOLATED research environment (never a system environment,
   never the platform runtime) and install the pin from
   `requirements-andes.txt` after verifying the current ANDES release
   (RESEARCH.md §7 task 5) and running the P8.36 at-pin-time CVE check.
3. Run the pure self-tests (no ANDES needed):
   `python apps/worker/python/andes_worker/evaluation_harness.py`
   — prints one PASS/FAIL line per hand-computed self-test.
4. Run the pytest suites (no ANDES needed for the pure tests; the ANDES
   path tests skip loudly where the guard demands the real package):
   `PYTHONPATH=apps/worker/python/andes_worker python -m pytest
   apps/worker/python/andes_worker/tests -v`
5. Execute a study end to end (the P8.17 human gate):
   pipe the request document (case, models, monitored signals, event,
   evaluation window) into
   `python apps/worker/python/andes_worker/andes_dynamic_worker.py`
   and record the result envelope in `17_dynamic_evaluation.md`'s EMPTY
   cells — the filling commands are recorded there verbatim.

## Honest limitations

- Nothing in this directory has ever executed; the ANDES API usage is
  UNVERIFIED until the first run against the pinned version, and the
  signal-to-array trace mapping (`extract_traces`) is explicitly a
  VERIFY-BUILD item: an unmappable signal is recorded as unexported, never
  filled with another signal's values.
- The eigenvalue result surface (`system.EIG.eval`) and the TDS recorded
  output surface (`system.dae.ts`) are read defensively; their exact shapes
  are VERIFY-BUILD items, and their absence is an in-band failure.
- The gRPC binding of the generated `flexcore.worker.v1` servicer is the
  recorded activation seam (Phase 7 template pattern); until then the
  transport is the grid-worker-style JSON document discipline — the gap is
  recorded in `ledger/phase-08-advanced-coordination/16_andes_integration.md`
  §7 as a Phase 7 follow-up, never fixed by inventing a parallel contract.
