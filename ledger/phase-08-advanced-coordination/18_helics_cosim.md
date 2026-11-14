# P8.18 — Co-simulation integration (HELICS) for multi-domain interaction

Date: 2026-11-13. Subtask: P8.18 (`ledger_phase_8.md` §P8.18).
Owner role: Research engineer. Report trace: §19.1 (multi-domain
co-simulation track); §7.4 (HELICS [R16]). Inputs: P8.16 dynamic
integration (`16_andes_integration.md`); P8.01 simulator adapters; the
Phase 5 HELICS scaffold (`apps/worker/python/helics_federates/` —
`federate_common.py`, `control_federate.py` pattern,
`requirements-helics.txt`); Phase 5 RESEARCH.md §1 (HELICS 3.7.0,
released 25 Aug 2026, BSD-3-Clause **found/reported**); Phase 8
RESEARCH.md §5 (framework role verified; version open task); gate G-6
(`01_readiness_check.md`: pyhelics binding match unconfirmed — Phase 5
open task 3).

## 1. Status of this record — federate WRITTEN, NEVER RUN; nothing installed

The research controller federate
(`apps/worker/python/helics_federates/research_controller_federate.py`)
was **WRITTEN, NEVER EXECUTED**: `helics==3.7.0` was **NEVER installed**
in this repository's history, no broker was started, no federation was
formed, and no scenario run record exists. The pin is **carried from
Phase 5 and labelled UNVERIFIED — RE-VERIFY at first install**:
re-check the HELICS release current at implementation, CONFIRM the
matching `pyhelics` binding version (Phase 5 RESEARCH open task 3 / gate
G-6, still OPEN), and run the P8.36 at-pin-time advisory check — the
archived `helics-apps` package must NOT be used (Phase 5 RESEARCH §1).
The BSD-3-Clause license remains **found/reported**, not formally
audited (P8.03 §2 carries it as such). Nothing in this record is evidence
that any co-simulation ever ran. The tick on P8.18 means *implemented,
awaiting human build/test*.

## 2. Federation definition (ledger step 1) — the research extension

The research federation **extends** the P5.16 three-federate minimum
(asset / grid / control, topics and units per
`federation_config.example.json`); it does not fork it. The added leg:

```text
research_controller_federate --proposal_p_mw (MW)--> (grid/asset leg, as configured)
asset/grid federates         --p_mw (MW), q_mvar (MVAr), vm_pu (pu), loading_percent (percent)--> research_controller_federate
```

| Federate | Role | Script | Exchanges | Time step |
|---|---|---|---|---|
| `flexcore_asset` | `asset` | existing `asset_federate.py` | Phase 2 simulator records (MW/MVAr); receives control setpoints | pinned federation `time_step_seconds` |
| `flexcore_grid` | `grid` | existing `grid_federate.py` | drives the UNMODIFIED P5.15 pandapower worker; publishes vm_pu / loading_percent | same |
| `flexcore_control` | `control` | existing `control_federate.py` | publishes schedule-fixture setpoints; subscribes measurements | same |
| `flexcore_research` | `research_controller` | **NEW** `research_controller_federate.py` | publishes `flexcore/research/<asset_key>/proposal_p_mw` (MW, injection-positive); subscribes measurements | same |

**The multi-domain scope (comms/market/end-use) is declared, not
built.** The P8.18 objective names grid, communication, market, and
end-use simulators; the end-use/asset and grid legs exist from Phase 5,
and the research leg delivers proposal values whose DELIVERY delay a
communication-domain federate would model. A communication federate and
a market federate are **not implemented in this step** — they are
recorded as the federation's extension plan with EMPTY participation
cells (nothing exists to run); inventing stub federates would be
fabrication, not implementation. The first multi-domain scenario (a
communication delay affecting a research proposal's delivery to the grid
model) is defined in §4 and is **UNRUN**.

**Proposal topic discipline:** the research federate publishes ONLY on
the `flexcore/research/.../proposal_p_mw` family — a distinct family from
the control federate's `flexcore/control/.../setpoint_p_mw` — so a
research proposal can never be mistaken for, or silently substituted
into, a control setpoint stream. `P > 0` = export at the point of
connection (§10.2).

## 3. Research-controller federate — the code boundary

`research_controller_federate.py` mirrors the Phase 5 federate pattern
(`federate_common.py` reuse: config loading, fail-closed `fail()`, the
guarded `import_helics()` gate, exit-report emission; helics import
INSIDE the flow with the named ImportError; every helics call
UNVERIFIED per the shared API table):

- **Observes/publishes data only:** publishes the pre-computed proposals
  fixture's active interval per step (absence → skip, never a fabricated
  proposal) and records received measurements as federation VALUES.
- **NO field-write path:** the script's only output channels are HELICS
  publications and the one exit-report document on stdout; there is no
  command lifecycle state, no dispatch call, and no authorization gate
  anywhere in it (ADR-010; §13.4).
- **Mode-locked simulation/shadow:** the constant `MODE_LOCKED` travels
  in the exit report (`control_authority_mode_locked`), alongside
  `field_write_path: "absent"` and `proposal_only: true`; no request,
  plugin, or research module can select or raise a control-authority
  mode (§21.4, §5.7).
- **Research-only role check:** refuses any config section whose role is
  not `research_controller`; refuses publication topics outside the
  research family and subscriptions outside the asset/grid measurement
  families (fail-closed, exit 2, nothing on stdout).

## 4. Scenario run record (ledger step 3) — EMPTY TEMPLATE

| Field | Value |
|---|---|
| Scenario | Communication delay affecting a research proposal's delivery to the grid model (the ledger's example), on the research federation of §2 |
| Federation config used | `RECORD-AT-FIRST-RUN` (a scenario-level versioned JSON document, the P5.16 config discipline extended with the research federate section) |
| HELICS version installed/reported | `RECORD-AT-FIRST-RUN` (pin 3.7.0 carried UNVERIFIED) |
| pyhelics binding version | `RECORD-AT-FIRST-RUN` (G-6 OPEN) |
| Federate exit reports (granted steps, final granted times, counts) | EMPTY — never produced |
| Cross-checked time synchronisation (equal granted steps / final times) | EMPTY — the P5.16 orchestrator check never ran |
| Measured interaction effect (federated vs interaction-disabled, §5) | **EMPTY — never measured** |
| Outcome / notes | EMPTY |

**Filling command (recorded verbatim, for the human pass):**

```sh
# 1. Install helics==3.7.0 (AFTER re-verifying the current HELICS release,
#    the pyhelics binding match [gate G-6], and the P8.36 advisory check)
#    into an ISOLATED environment.
# 2. Compose/validate the federation config WITHOUT executing (the
#    orchestrator's composed mode; NOTE §6 gap — see below):
#    flexcore-sim-runner scenario-run --scenario <path> \
#      --helics-federation <research-federation-config.json>
# 3. Execute:
#    flexcore-sim-runner scenario-run --scenario <path> \
#      --helics-federation <research-federation-config.json> --helics-execute
# 4. Record every federate exit report and the orchestrator's run record
#    into §4's cells; then run the interaction-disabled control (§5) and
#    record BOTH before any comparison sentence is written.
```

## 5. Boundary statement and the baseline-comparison record (ledger steps 4–5)

**Boundary statement (verbatim posture):** HELICS **synchronizes
federates** — it grants time steps and carries publications/
subscriptions. **It is not itself the power-system physics model**
(§19.1): the physics stay in the federates' models (the Phase 2 asset
simulator profiles, the Phase 4 pandapower AC power-flow pipeline, the
P8.16 ANDES dynamic worker). A HELICS run record is a co-simulation
transport record, never a power-system result; "the federation ran" says
nothing about grid behaviour. This is the same posture the Phase 5
scaffold records in `federate_common.py` (`HELICS_ROLE_STATEMENT`) —
unchanged and inherited here.

**Why no deterministic-baseline comparison applies to the federation
itself (ledger step 5):** HELICS is the COORDINATOR, not a decision
module — it computes no schedule, no proposal, and no physics (§19.1), so
there is nothing for a deterministic-baseline module to beat; a
"baseline HELICS" is a category error, not a missing experiment.

**The comparison that DOES apply (defined, UNRUN):** the same
multi-domain scenario run **federated vs unfederated** (or with the
interaction disabled — e.g. the proposal delivered without the
communication-domain delay), so the measured INTERACTION EFFECT has a
control. Both arms' cells in §4 are EMPTY; neither arm may be filled
without the other, or the "interaction effect" is an uncontrolled
number.

## 6. Contract gaps → Phase 7/5 follow-ups (recorded, not invented)

1. **No research role in the P5.16 orchestrator (Phase 7/5 seam):**
   `apps/sim-runner/src/helics_orchestration.rs` validates exactly three
   roles (`asset`/`grid`/`control`) and their publication families; the
   `research_controller` role and the `flexcore/research/...` topic
   family are not among them, so running `research_controller_federate.py`
   under that orchestrator requires an **additive** role variant there
   (enum entry, per-role option validation, canonical-unit/topic
   allowance). Per the ledger's deliverable, NO sim-runner seam was added
   in this step; the additive change is the recorded follow-up for the
   orchestrator's owner — a contract gap, never papered over by a
   parallel orchestrator.
2. **pyhelics binding pin (gate G-6, carried):** `requirements-helics.txt`
   names the co-simulation version; the binding package/version must be
   CONFIRMED at implementation before the first run (Phase 5 open task 3).
3. **Message-level time-sync audit (carried from the P5.16 README):** the
   granted-time/step-count cross-checks are the recorded
   time-synchronization scheme; a deeper message-timestamp audit remains
   a future task, not claimed here.

## 7. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| Federation runs with all named federates | **UNRUN** — federate written, never executed; helics never installed |
| The scenario's interaction effect measured and recorded | **EMPTY** (§4) — both arms of the §5 control defined, never run |
| Boundary statement present | §5 (and carried in the federate's exit report via `research_note`) |
| Versions pinned | HELICS 3.7.0 carried UNVERIFIED-REVERIFY; pyhelics OPEN (G-6); federate-script versions recorded at first run |

## 8. Verification tools (permitted only; run after this record was written)

`python tools/verify_docs.py`, `python tools/validate_agent.py`,
`python -I tools/check_dependency_rule.py` — actual output quoted in the
agent completion report. No install, build, execution, federation run,
or advisory lookup was performed for this integration.
