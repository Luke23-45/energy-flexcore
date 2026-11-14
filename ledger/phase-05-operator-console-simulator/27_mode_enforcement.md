# Decision record — P5.27 Control-authority mode enforcement in the console (§21.4, §17.5, §5.7)

**Status:** implemented, awaiting human build/test. **Nothing was built,
installed, run, or executed** — no cargo command, no npm install, no vitest
run (loop bans; AGENTS.md working agreement). Every test below is written
and pre-registered UNRUN; the human build/test pass is the gate.

## 1. What was built

| Artefact | File | Content |
|---|---|---|
| Mode control module (NEW) | `console/src/security/modeControl.ts` | The mode provider + gate: the closed §21.4 four-mode vocabulary (`Simulation` / `Read-only/shadow` / `Operator-approved` / `Bounded automation`), the exercised-mode set (Simulation, Read-only/shadow — the 0.x line), the deployment configuration surface (`DEPLOYED_CONTROL_AUTHORITY_MODE` + the optional `DEPLOYMENT_SITE_MODES` per-site override map), `effectiveMode` (per-site resolution, fail-closed on a defective name), `gateActionForMode` (the pure per-mode gate over the mirrored action surface, deny-by-default off the mirror), `combineChecks` (the capability ∩ mode intersection, both reasons rendered), `useCanInMode` (the combined hook over the claims-derived session), and the verbatim honesty notes (`MODE_ENFORCEMENT_NOTE`, `MIXED_MODE_NOTE`, `NON_EXERCISED_MODES_NOTE`). |
| Mode gate notice (NEW, shared component) | `console/src/components/modeGateNotice.tsx` | ADDITIVE beside the P5.13 `ControlAuthorityModeIndicator` (which is NOT modified): the deployed mode, the closed vocabulary with exercised vs configured-but-not-exercised chips, the defence-in-depth boundary statement, the per-site (mixed-mode) rule, and the no-promotion statement. Distinct markers + text, never colour alone (P5.12). |
| Mode gate tests (NEW) | `console/src/security/modeControl.test.tsx` | 16 tests (UNRUN): the closed vocabulary + the P5.13-constant equality pin, per-mode gate behaviour (Simulation allow-with-SIMULATED-qualification, Read-only/shadow deny-with-explanation, the two promoted modes configured-but-not-exercised with no promotion claim), deny-by-default off the mirror, the mode-vs-capability intersection (both-allow / either-deny, both reasons asserted), no-session denial, per-site resolution + defective-override fail-closed, and the honesty statements' render (including the `ModeGateNotice` render test). |
| Dispatch gate wiring (MINIMAL EDIT) | `console/src/workspaces/Dispatch.tsx` | The approval flow's gate composes the mode check WITH the capability check: `combineChecks(capability ?? hookCheck, gateActionForMode(APPROVAL_ACTION_ID))` — the console's one mutating action flow now requires BOTH the recorded grant (P5.26) AND the mode (P5.27), with both denial reasons rendered. No other line of the file changed. |
| Simulation Lab mode display (ADDITIVE) | `console/src/workspaces/SimulationLab.tsx` (P5.08 owner) | `<ModeGateNotice />` rendered beside the mode indicator; covered by the P5.08 test file's mode-interaction tests. |

No `apps/control-plane/**`, no `crates/**` file was touched: no mode-
serving endpoint exists and none was invented.

## 2. The enforcement-honesty record (the core of this subtask)

Rendered verbatim as `MODE_ENFORCEMENT_NOTE`, and recorded here:

1. **Where the mode comes from.** A source scan of `apps/control-plane/src`
   at authoring time found NO control-authority-mode endpoint or served
   configuration surface (the only "authority" hits in the tree are
   comments in `crates/flexcore-application/src/command_record.rs` and
   `command_gates.rs` stating that a plugin can never select the mode,
   §5.7). No mode-reading route exists in this phase and NONE WAS
   INVENTED. The mode therefore comes from the console's DEPLOYMENT
   configuration (`DEPLOYED_CONTROL_AUTHORITY_MODE`, changed per
   deployment, never per user/session/URL/request/plugin), and the SERVER
   REMAINS AUTHORITATIVE: the console constant is the defence-in-depth
   mirror of a deployment setting, never the setting itself.
2. **What the console gate is.** Defence-in-depth UI, NOT the enforcement
   point. The API enforces the boundary independently: the P4.16 guard
   (`SimulatorDispatchAdapter`) hard-refuses every non-simulator target
   kind (re-verified in `29_safety_invariant_matrix.md` invariant 6), and
   the console's own REST surface is read-only (§15.2 — no generic execute
   endpoint exists), so the console has NO non-simulator issue surface at
   all. This is the P5.26 criteria wording applied to the mode ("the API
   enforces it too — the console hiding a button is not the enforcement")
   and the P5.27 criteria wording ("the API rejects such attempts
   independently (P4.16 guard)"; the ledger's recorded risk "a mode flag
   in the console that the API ignores" is mitigated server-side, not by
   the console).
3. **The P4.32 §2 boundary** travels on the gate display verbatim (it is
   quoted inside `MODE_ENFORCEMENT_NOTE`): every issued command is issued
   to the SIMULATOR and nothing else; acknowledgement is NOT physical
   delivery; operator-approved field control is v1.0.0, not now; real
   protocol adapters are Stage 6.
4. **Promoted modes are configured-but-not-exercised.** Operator-approved
   and Bounded automation render as the §21.4 configuration vocabulary
   only: the entire 0.x line is simulation-only, mutating actions are
   DENIED under them, and no promotion path exists in this phase
   (promotion requires the §22.3 release gates plus site-specific
   acceptance — recorded, not available; P5.27 step 4).

## 3. Per-mode gate semantics (P5.27 steps 1–2)

| Mode | Mutating actions | Reads | Reason rendered |
|---|---|---|---|
| Simulation | ALLOWED — only against the simulator (the console has no non-simulator issue surface; the P4.16 guard refuses every non-simulator target; every result SIMULATED-labelled) | allowed | the SIMULATED qualification + the structural no-non-simulator-surface boundary |
| Read-only/shadow | DENIED with the explanation (never silently hidden) | allowed | shadow computes and compares WITHOUT issuing commands (§21.4); the API rejects independently |
| Operator-approved | DENIED — configured-but-not-exercised | allowed | configuration vocabulary only; 0.x simulation-only; no promotion path |
| Bounded automation | DENIED — configured-but-not-exercised | allowed | same |
| Defective mode configuration | DENIED (fail-closed) | — | the defect named, never coerced |
| Action id off the mirror | DENIED in every mode | — | deny-by-default (P1.11); the API would deny with UnknownAction anyway |

The intersection rule: a console action proceeds only when the P5.26
capability check AND the P5.27 mode check BOTH allow; both denial reasons
render; the wired site is the Dispatch approval flow (`command.approve`).
Reads are permitted in every mode — a mode constrains control authority,
never evidence reading.

## 4. Mixed-mode display (P5.27 step 3)

Mode is a deployment AND per-site setting (§21.4). `effectiveMode(siteId)`
resolves the per-site override from `DEPLOYMENT_SITE_MODES` (null = uniform
deployment — the recorded configuration in this phase is uniform
Simulation). The resolution path is implemented and tested so a mixed
deployment fills only the constant, never new code; the note renders the
per-site display rule and states that this deployment is uniform. No mode
is invented for a site that has none.

## 5. Decisions (all DEFAULT unless noted)

| ID | Decision | Label |
|---|---|---|
| D-P5.27-1 | The deployed mode is a deployment-configuration constant in modeControl.ts that MUST equal the P5.13 display constant (`CONSOLE_CONTROL_AUTHORITY_MODE`); the equality is pinned by a test rather than by refactoring honesty.tsx (which the P5.13 wave owns and other tests pin). A mismatch is a defect the tests refuse to hide. | DEFAULT |
| D-P5.27-2 | No mode-serving API is invented: the control-plane scan is recorded (§2.1), the constant is the deployment surface, and the server-remains-authoritative statement renders wherever the gate is displayed. | DEFAULT |
| D-P5.27-3 | Action classification derives from the capability mirror (`.read` suffix = read; everything else on the mirror = mutating) with deny-by-default off the mirror — one action surface, no second list to drift. | DEFAULT |
| D-P5.27-4 | The promoted modes (Operator-approved, Bounded automation) deny mutating actions as configured-but-not-exercised rather than simulating their semantics: rendering behaviour the 0.x console does not have would be a fabricated capability. | DEFAULT |
| D-P5.27-5 | The gate wires into the Dispatch approval flow only (the console's one mutating action surface, where useCan is already wired); the shared `ModeGateNotice` component and `useCanInMode` are the adoption path for any future mutating surface. SimulationLab is read-only and renders the notice; no other workspace has a mutating action to gate. | DEFAULT |
| D-P5.27-6 | Reads are allowed in every mode (including the promoted vocabulary): §21.4 modes constrain control authority, not evidence reading, and the console's reads are tenant-scoped and API-enforced (P5.26). | DEFAULT |

## 6. Honest limitations

1. **Everything is UNRUN.** No vitest run, no build, no type-check has
   executed. The 16 tests are pre-registered, not passing.
2. **The console gate is not the enforcement.** It cannot stop a forged
   client; the server-side gates (P4.16 guard, the read-only §15.2
   surface, the P4.13 authorization-gate records, the P4.28 denial audit)
   are the enforcement and are cited, not duplicated. The ledger's
   mitigation ("API rejection is tested independently") lives in the
   Phase 4 suites.
3. **The deployment configuration is a compile-time constant.** A runtime
   deployment-config file (or a future control-plane mode endpoint) is the
   later-stage replacement; the constant's doc comment records the
   migration point, and the equality test with the P5.13 constant keeps
   the two surfaces honest until then.
4. **`DEPLOYMENT_SITE_MODES` is null in this phase** (uniform
   Simulation). The per-site resolution is implemented and tested at the
   pure level; no mixed-mode deployment fixture exists because no
   deployment does this yet — the display rule and the fail-closed
   defective-override rule are what the tests pin.
5. **Pre-existing UNRUN inconsistency (found, not owned here):** the
   Dispatch render tests pass `claims={null}` and expect the confirmation
   checkbox, but the P5.26 gate (and now the composed mode gate) renders
   the denial block without a session — the contradiction predates this
   subtask (wave 5b landed the gate; the tests never ran). Recorded in
   `08_simulation_lab.md` §5.6 as well so the human's build pass is not
   surprised.
