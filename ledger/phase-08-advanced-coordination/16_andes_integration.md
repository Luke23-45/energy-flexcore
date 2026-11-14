# P8.16 — Dynamic simulator integration (ANDES): scope, models, and the steady-state boundary

Date: 2026-11-13. Subtask: P8.16 (`ledger_phase_8.md` §P8.16).
Owner role: Research engineer (power systems). Report trace: §19.1
(dynamic stability track); §7.4 (ANDES); §13.2–§13.3 (stability research
context). Inputs: P8.02 research plan; P8.03 license audit
(`03_research_license_audit.md` §3 — ANDES GPLv3 isolation design, gate
G-4 OPEN); Phase 8 `RESEARCH.md` §4 (ANDES 2.0.0, R8-C5); Phase 5
`grid_worker.py` transport pattern; Phase 7 worker template
(`python/workers/worker_template/`).

## 1. Status of this record — code WRITTEN, NEVER RUN; nothing installed

ANDES **2.0.0** (PyPI, released 11 March 2026; RESEARCH.md §4) is
**GPLv3** and has **NEVER been installed** in this repository's history;
the worker below was **NEVER executed** — not against real ANDES, not
against a stub. Every `andes` call follows the DOCUMENTED ANDES 2.0 API
as of the 2026-11-07 research cut-off and is **UNVERIFIED** until the
human build/test pass runs it (R8-C5: pin 2.0.0 **or a VERIFIED later
release** at implementation; the live docs show a development version
string and are never a version citation). Nothing in this record is
evidence that any dynamic simulation ever ran. The tick on P8.16 means
*implemented, awaiting human build/test* — nothing more.

Gate G-4 (ANDES GPLv3 isolation **legal confirmation**) is **OPEN**
(`01_readiness_check.md` §gates). This record implements the isolation
design; it cannot confirm it.

## 2. Scope definition (ledger step 1)

| The dynamic simulator ANSWERS | The dynamic simulator does NOT answer |
|---|---|
| Does a proposed research controller (an ADMM/DMPC or MARL **proposal**, applied to the simulated system in shadow) affect frequency/transient behaviour **in the declared ANDES model and case**? | Any statement about the real grid's frequency or transient stability — the model is a declared model, not the grid (§19.1) |
| Eigenvalue analysis (small-signal mode structure) **of the declared, linearized model** | Distribution quasi-static feasibility — that is the Phase 4 pandapower AC power-flow pipeline's question (§12.4/§12.5); the two scopes are never merged |
| Per-disturbance traces (frequency, voltage) for the P8.17 evaluation template — every measured cell filled only by a human/lab run | Protection behaviour, relay coordination, inverter inner-loop or sub-cycle control — FlexCore is supervisory (non-negotiable rule 3); ANDES models are simulation models, not field evidence |
| Whether a proposal drives the declared model unstable (instability is a recorded observation about the model) | Safety, certification, or per-feature qualification of anything (§2.1; P8.29 records that none is granted) |

Non-claims carried verbatim in every worker result (`BOUNDARY_STATEMENTS`
in `andes_dynamic_worker.py`): model-scoped statements only; steady-state
PF ≠ transient stability; proposals only (ADR-010); mode-locked
simulation/shadow; GPLv3 isolation with G-4 open.

## 3. Model and case record (ledger step 2) — STATIC, nothing run

**Tool pin:** ANDES **2.0.0** (`apps/worker/python/andes_worker/
requirements-andes.txt`; RESEARCH.md §4, R8-C5). The worker reports the
installed `andes.__version__` defensively at run time; the pin discipline
and the P8.36 at-pin-time advisory check live with the human gate, never
with a silent mismatch refusal.

**Model scope (recorded from RESEARCH.md §4, checked against the ANDES
docs at the cut-off):** generator models GENROU/GENCLS; governors
TGOV1/IEEEG1/HYGOV; exciters EXST1/ESST3A/SEXS/AC8B; PSS IEEEST/ST2CUT;
renewables REGCA1/REECA1/REPCA1/WTDTA1; DER models **PVD1/ESD1**; loads
ZIP/FLoad. The worker request declares its `models` list per study and
the result carries it verbatim — a study never runs against an
undeclared model scope.

**Case candidates (RECOMMENDATION to verify at first install — recorded
ASSUMPTION, not a fact):** Kundur two-area and IEEE 14-bus with dynamic
data, as ANDES sample cases. That ANDES ships these exact bundled cases
was NOT verified in this loop; the human pass records the actual case
file paths and their ANDES-version provenance in `case_version` per
request (the worker refuses an empty `case_version`).

**Solver settings:** recorded per study in the request
(`simulation.solver`, `step_seconds`, `end_time_seconds`) and echoed into
the result's `provenance.solver_version`/model scope — the ledger's risk
("an ANDES version/model change unrecorded") is met by refusing studies
without a declared scope, not by defaults.

## 4. Implementation and the six-point isolation (ledger step 3)

The integration is an isolated Python worker
(`apps/worker/python/andes_worker/andes_dynamic_worker.py`), reusing the
Phase 7 template's guarded-import and §9.2-provenance patterns and the
Phase 5 grid worker's one-JSON-document-in/one-out transport, with
protocol tag `flexcore-andes-worker/1`. ANDES never appears anywhere
else. The P8.03 six-point design, mapped to code:

| # | Isolation rule (P8.03 §3) | Where it is implemented / verified |
|---|---|---|
| 1 | ANDES lives ONLY inside an isolated Python research worker (Class C) | `apps/worker/python/andes_worker/andes_dynamic_worker.py` — the only `import andes` in the repository, deferred INSIDE `import_andes()`; a missing ANDES raises the NAMED `ANDES_DEPLOYMENT_REQUIREMENT` (which names G-4), so the pure-logic core imports and tests without ANDES — and no study result can exist without ANDES having actually run |
| 2 | No core crate, SDK crate, or app depends on ANDES; never linked, vendored, or FFI-bridged | No Rust change was made in this step; `requirements-andes.txt` records that ANDES must NEVER enter the cargo tree (`deny.toml` keeps GPLv3 disallowed by construction) |
| 3 | FlexCore consumes ANDES **outputs** as data, never ANDES-derived code | The result envelope carries traces/eigenvalues/metrics as JSON data over the worker boundary; metrics are computed by the stdlib-only pure `evaluation_harness.py` on injected series — no ANDES type crosses the boundary |
| 4 | Any distribution of ANDES-derived code goes through legal first | Recorded here; the worker/README repeat it; G-4 remains the human gate |
| 5 | The GPLv3 boundary is verified by INSPECTING the worker boundary | The import-graph check is §6 below (the evidence procedure, to be run by the reviewer) — not this table |
| 6 | Dynamic results cross the trust boundary as UNTRUSTED INPUT, never dispatch inputs | Proposal-only in code: `assert_proposal_only()` scans the whole result tree for authorization/command-shaped keys and refuses emission; the response type carries `authority: "none"` and no command field exists |

**Proposal-only enforcement is in the response type, not prose** (P8.04
posture): the forbidden-key scan plus the absence of any token/command
field from `build_result` means a research result cannot express an
authorization token. The worker is mode-locked to simulation/shadow by a
constant; no request field can change it (§21.4: mode is a
deployment/per-site setting).

## 5. Boundary statement (ledger step 4)

1. **A steady-state power flow is NOT transient-stability evidence.** The
   Newton–Raphson power flow ANDES runs for TDS initialization (and as
   the `steady_state_power_flow` study type) evaluates a steady state of
   specified injections; a converged, within-limits case says nothing
   about frequency nadir, ROCOF, or transient response (§19.1). This
   sentence travels verbatim in every worker result.
2. **The dynamic simulator is NOT a replacement for distribution
   quasi-static power flow.** Feasibility gates stay on the Phase 4
   pandapower pipeline; dynamic results are never quasi-static feasibility
   evidence, and quasi-static results are never dynamic evidence
   (§19.1; the ledger's named risk, closed structurally by separating
   the tools and the boundary statements).
3. **Solver-class discipline (AGENTS.md rule 4):** no claim here concedes
   or implies that a time-domain simulation is an optimization, that an
   eigenvalue analysis is an OPF, or that any result carries global
   optimality. ANDES is a simulator; it is nobody's solver backend.
4. **ANDES never writes to any device and has no field-write path.** The
   worker's only output channel is one JSON document on stdout; its
   results are proposals/diagnostics (ADR-010), mode-locked to
   simulation/shadow. There is no path from an ANDES result to a command,
   and none may be added.
5. **Every dynamic figure is model-scoped.** Results about the declared
   model are stated as such; the P8.17 template attaches model/case
   references to every cell it will eventually hold.

## 6. Isolation verification procedure (the import-graph check — for the reviewer)

The ledger's verification method ("inspect the worker boundary for GPLv3
isolation") is executed as follows, by the human/reviewer:

```sh
# (a) the ONLY andes import in the repository, and it is deferred+guarded:
grep -rn "import andes" apps crates python --include=*.py --include=*.rs
#     expected: exactly one hit, inside
#     apps/worker/python/andes_worker/andes_dynamic_worker.py (function body)

# (b) no ANDES/HELICS/torch equivalent in the Rust tree:
grep -rni "andes" apps --include=Cargo.toml crates --include=Cargo.toml
#     expected: no hits (denied by construction; deny.toml has no GPL family)

# (c) the worker's pure core imports without ANDES installed:
PYTHONPATH=apps/worker/python/andes_worker python -c \
  "import andes_dynamic_worker, evaluation_harness; print('pure core OK')"

# (d) without ANDES, no result can exist (the named requirement):
PYTHONPATH=apps/worker/python/andes_worker python -c \
  "import andes_dynamic_worker as w; w.run_study({'protocol':'flexcore-andes-worker/1'})"
#     expected: ImportError naming ANDES_DEPLOYMENT_REQUIREMENT and G-4
```

Outputs of (a)–(d) are quoted into the human build/test pass record; the
check at step 3 of the ledger's acceptance ("worker runs ANDES
time-domain simulation and eigenvalue analysis on the recorded cases")
is **UNRUN** and belongs to that pass together with the P8.17 filling
commands.

## 7. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **gRPC binding seam (P8.04/P7.17):** the worker reuses the template's
   §9.2 provenance field names verbatim and its guarded-import pattern,
   but binds no generated `flexcore.worker.v1` servicer (buf generation
   remains an activation step; the transport is the grid-worker-style
   JSON document discipline). Binding the servicer into a Class C
   research worker is the recorded activation task — a Phase 7/P8.04
   seam, NOT a parallel contract.
2. **Research payload types:** `worker_api.proto` has no
   dynamic-study payload type; the JSON request/result shapes here are
   the worker's documented surface pending P8.04's contract work. If
   P8.04 defines research payload types, this worker's shapes migrate to
   them; nothing here pre-empts that decision.
3. **Unit vocabulary:** the worker's allowed monitored-signal units add
   `Hz`/`rad` to the §10.2 canonical set (frequency is not a MW-family
   unit); the Phase 5 HELICS orchestrator's canonical-unit set (MW, MVAr,
   MVA, MWh, kV, A, pu, percent) does not include them. If dynamic
   traces ever cross the HELICS transport, that set needs an additive,
   recorded extension — recorded here as the owner's follow-up, not
   changed unilaterally.

## 8. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Worker runs ANDES TDS + eigenvalue analysis on the recorded cases | **UNRUN** — code written, ANDES never installed; the named import gate proves the negative (no ANDES → no result) |
| Model/case versions recorded | Recorded STATICALLY (§3); per-run `case_version`/`andes_version_reported` carried by the worker (UNVERIFIED surface) |
| Boundary statement present | Present verbatim in every result envelope (`BOUNDARY_STATEMENTS`) and in this record (§5) |
| ANDES isolation (P8.03) verified by inspection | Procedure recorded (§6); the reviewer's run of it is the evidence — not this record |

## 9. Verification tools (permitted only; run after this record was written)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — actual output quoted in the
agent completion report. No install, build, execution, or scan was
performed for this integration.
