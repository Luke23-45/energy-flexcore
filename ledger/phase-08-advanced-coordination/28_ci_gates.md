# P8.28 — CI gates for research code (WRITTEN-UNRUN — config written, never executed; the green-run record is a human gate input)

Date: 2026-11-14. Subtask: P8.28 (`ledger_phase_8.md` §P8.28). Owner role:
DevOps. Report trace: §22.2; §19.2; §9.3. Inputs: P8.04
(`04_worker_contracts.md` — the research contracts whose conformance suite
this wires); P8.20 (`20_reproducibility.md` — the rerun standard the
reproducibility-check stub references); P7.27 CI
(`.github/workflows/ci.yml` — the Phase 7 additive-job pattern this
extends, including the recorded Python-gate seam). ENTRY_NOTE loop bans:
no CI run, no pytest, no cargo command — the yaml is WRITTEN-NEVER-EXECUTED.

## 1. Status of this record — WRITTEN-UNRUN

Four ADDITIVE jobs were appended to `.github/workflows/ci.yml`
(the Phase 7 jobs unchanged; the header comment block documents the
additions per the P6.27/P7.27 convention). **Every job is UNRUN** — no
CI execution happened in the writing loop, and ledger step 4 (record a
green CI run on the phase branch) is a HUMAN GATE INPUT recorded UNRUN
(§6), never claimed. Two of the four jobs are **fail-closed by
construction today** (the recorded pin gap and the missing artifacts) —
that is the honest state, not a defect: a green research gate that
installed nothing and compared against nothing would be exactly the
"compares against itself" failure mode the ledger names.

## 2. What was implemented (ledger steps 1–3 → config)

### 2.1 The jobs (all additive; no existing job modified)

| Job | Ledger step | Content |
|---|---|---|
| `research-python-gates` | 1 | the §22.2 Python gates over the research workers: Ruff + mypy type checks + pytest over `apps/worker/python` (reproducibility, gnn_worker, marl_worker, andes_worker, portfolio_analytics, drill suites) and `python/workers/tests`. The FIRST step is the **fail-closed pin gate**: it fails the job naming the recorded re-verify procedure while `python/workers/requirements.txt` carries `==UNPINNED-REVERIFY` pins (grpcio/protobuf), while the DECISION-POINT files (`requirements-gnn.txt`, `requirements-marl.txt`, `requirements-andes.txt`) carry no pin line, and while no pytest/ruff/mypy pin exists. NO version is invented by CI (D4; P8.03; ENTRY_NOTE boundary 5) — the gap is RECORDED, and the post-gate steps (install, lint, type-check, the three pytest legs) run as written once the human lands verified pins |
| `research-rust-harness-gates` | 1 | the Rust gates for the P8.05 validation harness + P8.06 baseline registry: the dedicated `cargo test -p flexcore-research` verdict (fmt/clippy/workspace tests already cover the crate through the workspace-wide jobs — the P7.27 coverage-note pattern, restated not duplicated), the harness/registry version-record lines (the P7.27 pattern), and the **STRUCTURAL check of the frozen research benchmark artifact** (`benchmarks/research-set-v1.json`: `version_id`, in-set case rows present; `benchmarks/research-set-v1-not-tested.json`: list id, version coupling, 11 items) — structure only, never a result (no research run exists) |
| `research-worker-conformance` | 3 | the research-worker conformance suite (P7.21 extended to the research contracts per `04_worker_contracts.md` §6): the Python leg `python/workers/tests/test_research_contracts.py` via pytest, with the suite id recorded (`flexcore-plugin-sdk-conformance-suite-v1`, extended) and the Rust-side twins named (`crates/flexcore-research/src/validation.rs` cfg(test) suites — pinned by `research-rust-harness-gates`). Same fail-closed pin gate |
| `research-reproducibility-check` | 2 | the CI reproducibility check STUB referencing `20_reproducibility.md` §5/§8(f): a seeded benchmark run re-executed from its P8.20 record ALONE (second engineer, clean environment, identity check first — `INVALID_RERUN_IDENTITY` never a verdict) and compared against the **FROZEN P8.06 baseline results artifact** within the **PRE-REGISTERED** P8.38 tolerance (X-8 — injected, never derived). The readiness gate step FAILS today with the recorded UNRUN statement (no frozen artifact, no P8.38 pre-registration, no P8.20 record — every §7 cell EMPTY); the stub can never go green by comparing against its own output, and the recorded check procedure is documented in the job for the human to enable when the artifacts exist |

### 2.2 Ledger step mapping

| Ledger step | Status |
|---|---|
| 1 — Python Ruff + type checks + pytest for research workers; Rust gates for the harness; scenario regression including the research benchmark cases | the Python and Rust gates are wired (above). **The scenario-regression extension is RECORDED AS A GAP, not invented** (§5.1): the existing `scenario-regression`/`phase3-scenario-regression` jobs run the sim-runner suites; the research benchmark cases (`flexcore-research-benchmark-set-v1`) have never been run and have no executable suite in this loop — wiring them into a regression job requires the harness runs to exist first, and a job that runs nothing while asserting success would be a fake gate |
| 2 — the CI reproducibility check (seeded run vs the frozen artifact within the pre-recorded tolerance; never against its own output) | the fail-closed STUB is wired (above); the check itself is **UNRUN until the artifacts exist** — exactly the ledger's "check compares against the frozen artifact" rule, enforced structurally |
| 3 — the research-worker conformance suite as a CI job | wired (above), Python leg + named Rust leg |
| 4 — record a green CI run on the phase branch | **UNRUN** — human gate input (§6); never claimed from config |

## 3. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| All §22.2 gates run on the new research code and pass | the gates are WRITTEN; none has RUN — the fail-closed pin gate makes them un-greenable until the human pins verified versions (the recorded gap, not a workaround) |
| The seeded reproducibility check passes in CI | **UNRUN BY RECORDED STATE** — the artifacts it requires (frozen baseline results, P8.38 pre-registration, P8.20 records) do not exist; the stub fails with that statement rather than passing falsely |
| The research conformance suite runs in CI | the job is WRITTEN behind the same pin gate; UNRUN |

## 4. The verification method (the ledger's: break a seed/version pin and confirm CI fails)

The scratch-branch negative demonstrations are HUMAN GATE INPUTS, UNRUN:

1. break a version pin on a scratch branch (e.g. edit a `==` pin to a
   different version without the recorded decision) → confirm the
   fail-closed pin gate fails the job with the named procedure;
2. remove an item from the frozen not-tested list (or edit the version
   id) → confirm `research-rust-harness-gates`'s structural check fails;
3. after the artifacts exist, feed the reproducibility check a rerun
   whose identity fields mismatch the primary record → confirm
   `INVALID_RERUN_IDENTITY` fails the job (never a verdict).

## 5. Contract gaps → follow-ups (recorded, not invented)

1. **The research-stack pins are UNPINNED-REVERIFY / DECISION-POINT-OPEN**
   (P7.27 grpcio/protobuf seam; P8.03 §2/§4 torch/PyG, RLlib, ANDES):
   the Python gates cannot execute until the human re-verify procedures
   recorded in the requirements files resolve. No version is invented by
   CI or by this record.
2. **The scenario-regression leg for the research benchmark cases is not
   wired** (ledger step 1's third clause): the frozen cases have never
   been run and no executable research scenario suite exists in this
   loop; wiring one that asserts success over nothing would be a fake
   gate. The recorded activation path: the P8.05/P8.06 harness runs
   produce results → the research cases enter the regression suite in
   the activation wave, comparing against the FROZEN artifact.
3. **The reproducibility check is a stub until the artifacts exist**
   (P8.06 frozen results, P8.38 pre-registrations, P8.20 records): the
   readiness gate fails honestly; the recorded rerun command
   (`20_reproducibility.md` §8 (f)) is the procedure the human wires in.
4. **pytest/ruff/mypy pins are themselves unpinned** (recorded gap): the
   tool pins land at the first human build/test pass, never floated.
5. **No green run exists** (ledger step 4): the tick records
   "implemented, awaiting human build/test" per ENTRY_NOTE; a green-run
   claim without the recorded run is a finding against this record.

## 6. UNRUN register (the human gate inputs)

| Item | Status |
|---|---|
| any CI execution of the four new jobs | UNRUN — loop bans |
| the green CI run on the phase branch (ledger step 4) | UNRUN — human gate input |
| the scratch-branch negative demonstrations (§4) | UNRUN — human gate input |
| the Python gates' first real execution (after the pins land) | UNRUN — gated on the recorded re-verify procedures |
| the reproducibility check's first real execution | UNRUN — gated on the frozen artifacts + P8.38 pre-registrations |

## 7. Reviewer-verification procedure

```sh
# (a) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the config-only structural check (no CI run):
#     1. confirm the four jobs are additive: the Phase 7 jobs
#        (plugin-sdk-conformance, sdk-scenario-regression,
#        plugin-manifest-validation, wit-validate, worker-contract-buf,
#        no-dylib-gate) are UNCHANGED;
#     2. confirm NO fabricated pin: grep the new jobs for a version
#        number attached to pytest/ruff/mypy/grpcio/torch/ray/andes —
#        every such slot must read the recorded RECORDED-AT-FIRST-HUMAN-
#        RUN / fail-closed form;
#     3. confirm the reproducibility stub compares against the FROZEN
#        artifact (never its own output): the job comment names the
#        comparison target and the identity-first rule;
#     4. the first human CI run is the verification point for every job
#        (the P6.27 run-record honesty pattern).

# (c) the ledger's verification method (UNRUN, §4): the scratch-branch
#     negative demonstrations, recorded here with their actual outputs
#     when the human runs them.
```

A green (a) plus the recorded human CI run earns only a one-line
"human build/test gate: PASSED (date)" appended here; failures are
recorded verbatim and the ledger tick reverts to `- [ ]`.

## 8. Non-claims (restated)

No CI job has executed; no gate has passed; no green run exists. The
fail-closed jobs failing is the recorded honest state, not a failure of
this record — and they cannot be made green by config alone. No
dependency version is recorded or suggested by this record or the yaml
(the pins are the recorded UNPINNED-REVERIFY / DECISION-POINT states,
verbatim). The reproducibility check does not exist as an executed
check — it is a stub whose readiness gate states exactly why. No
research benchmark case has run in CI or anywhere else. Nothing here
qualifies any feature for field use; the entire configuration governs
simulation-only research artifacts (AGENTS.md rule 5).
