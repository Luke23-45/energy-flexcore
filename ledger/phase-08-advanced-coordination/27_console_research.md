# P8.27 — Operator console: research and shadow-mode surfacing (WRITTEN-UNRUN — code + tests written, never built, never executed)

Date: 2026-11-14. Subtask: P8.27 (`ledger_phase_8.md` §P8.27). Owner role:
frontend engineer. Report trace: §20.1 item 9 (Simulation Lab); §20.2;
§19.2(9). Inputs: P8.14 (`14_marl_worker.md` + `apps/worker/python/
marl_worker/shadow_log.py` — the shadow logger whose statistics slots this
surface names); P8.23 (`23_cross_module_comparison.md` — the comparison
the shadow-comparison rows point at); Phase 5 console (the P5.22
Simulation Lab workspace this section extends); the P7.25 precedent
(`console/src/workspaces/PluginsWorkspace.tsx` + `api/plugins.ts` +
`apps/control-plane/src/routes_plugins.rs`/`dto_plugins.rs` — the
read-only-inventory pattern mirrored exactly). ENTRY_NOTE loop bans: no
npm install, no tsc/vite build, no vitest run — the code and its test
files are WRITTEN-NEVER-EXECUTED.

## 1. Status of this record — WRITTEN-UNRUN

The console changes exist as WRITTEN code (below); nothing was built and
no test was executed. **No research run exists anywhere in Phase 8**, so
the surface serves the RECORDED Phase 8 research status: every evaluation
record is a TEMPLATE with EMPTY cells, every shadow comparison is
`NO_RUN`, and the proposal set is EMPTY — never a plausible proposal,
result, or statistic (the P7.25 honesty contract, applied to research).
The ledger's operator walkthrough (the verification method) and the
step-5 negative test are **UNRUN** — human gate inputs (§7).

## 2. What was implemented (ledger steps 1–4 → code)

### 2.1 Files created (the P7.25 new-files-over-rewrites pattern)

| File | Content |
|---|---|
| `apps/control-plane/src/dto_research.rs` | the `/api/v1/research/status` read DTOs: `ResearchModuleDto` (closed P8.20 module vocabulary, recorded frozen version stamps or `NOT_RECORDED`, `TEMPLATE_EMPTY`/`NO_RECORD` evaluation status, owning decision record, recorded OPEN blockers, the provenance-rules note), `ResearchProposalDto` (provenance triple: module/model version, input snapshot id, P8.05 validation outcome — rendered `VALIDATION_OUTCOME_MISSING` when absent, the refusal never a pass; lifecycle position ALWAYS `PROPOSED`, never executed), `ShadowComparisonDto` (frozen P8.06 baseline id, `NO_RUN` status, the module-specific statistics slot — `None` = the gap, never a rate over an empty denominator), `ResearchStatusDto` (modules + proposals + comparisons + the verbatim `simulation-shadow` mode lock + the honest surface note) |
| `apps/control-plane/src/routes_research.rs` | the `GET /api/v1/research/status` route (the P7.25 route pattern): the `ResearchStatusSource` seam trait + the shipped default `RecordedResearchStatus` serving the RECORDED Phase 8 status verbatim (read 2026-11-14 from the phase decision records) with every run fact an explicit gap — ZERO proposals, all comparisons `NO_RUN`; the recorded-drift interim auth gate (`telemetry.read`, the swap point for a closed-enum `research.status.read` action owned by flexcore-authz); GET-only, no mutation endpoint; unit tests pinning the closed vocabulary, the no-run-facts rule, the X-5 dynamic provenance, the proposal-side lifecycle, and the surface note |
| `console/src/api/research.ts` | the typed wire mirror (snake_case, the api/plugins.ts discipline) + `fetchResearchStatus` (GET only) + the closed vocabularies (`RESEARCH_MODULE_IDS`, `RESEARCH_EVALUATION_STATUSES`, `SHADOW_COMPARISON_NO_RUN`, `PROPOSAL_STATUS_PROPOSED`, `VALIDATION_OUTCOME_MISSING`, `RESEARCH_MODE_LOCK`) |
| `console/src/workspaces/researchWorkspaceData.ts` | the data layer (the pluginsWorkspaceData.ts pattern): `loadResearchStatus`, the verbatim mapping helpers (`moduleVersionOf`, `shadowComparisonOf`, `proposalOf`), `RESEARCH_PROPOSED_VS_EXECUTED_NOTE`, `PROMOTION_REQUEST_NOTE`, and the LOCAL promotion-request event log (the denialLog pattern — capacity 100, never sent anywhere) |
| `console/src/workspaces/ResearchWorkspace.tsx` | the `ResearchStatusSection` extension panel: mode-lock display, per-module articles (role, version stamps or the named gap, evaluation record, verbatim OPEN blockers, provenance rules), the shadow-comparison table (status verbatim, statistics ABSENT with the empty-denominator rule stated), the proposal list with per-proposal provenance (all labelled `[PROPOSAL — …; never executed]`), the honest zero-proposal empty state, the promotion-REQUEST affordance with the explicit-confirmation checkbox (ConfirmHighImpact pattern), the promotion-audit-trail honest gap, the freshness line |
| `console/src/workspaces/ResearchWorkspace.test.tsx` | the test file (the PluginsWorkspace.test.tsx pattern; WRITTEN-UNRUN): states distinguishable, vocabularies verbatim, no rate fabricated over an empty denominator, zero-proposal honesty, provenance per proposal, GET-only client, confirmation-gated request, loading/error honesty |

### 2.2 Minimal additive wiring (recorded, per the ownership rules)

| File | Additive change |
|---|---|
| `apps/control-plane/src/lib.rs` | the two `pub mod` registrations (`dto_research`, `routes_research`) with the P8.27 comment — strictly required for the new modules |
| `apps/control-plane/src/routes.rs` | one additive `.merge(crate::routes_research::research_router())` line in `stage1_router` (the P7.25 merge pattern) — required for the route to be served |
| `apps/control-plane/src/state.rs` | one additive seam field `research_status: Arc<dyn ResearchStatusSource>` + its `RecordedResearchStatus` default + the `with_research_status` composition builder (the `plugins_inventory` field pattern verbatim) — required by the seam pattern |
| `apps/control-plane/src/openapi.rs` | the additive path registration (`get_research_status`) + the four schema registrations (the P7.25 registration pattern) — the console's shape-mirror note pins the OpenAPI schema as authoritative |
| `console/src/App.tsx` | the additive `SimulationLabWithResearch` composition: the P5.22 `SimulationLabWorkspace` unchanged + `ResearchStatusSection` below it on the same `simulation-lab` route (the `PluginsIntegrationsWithInventory` pattern verbatim); the registry is NOT modified (the `simulation-lab` entry already exists) |

No other file was touched. No existing route, DTO, or workspace component
was modified.

### 2.3 Ledger step mapping

| Ledger step | Implementation |
|---|---|
| 1 — research proposals with provenance, shadow comparisons vs the deterministic baseline, the validation result per proposal | the DTOs carry exactly the provenance triple (module/model version, input snapshot id, validation outcome) per proposal; the shadow-comparison rows cite the frozen P8.06 baseline ids; TODAY the surface serves the honest EMPTY state (no run exists) — a deployment composes a real source via `with_research_status` when runs exist |
| 2 — label every research output as a proposal/simulation result, visually distinct from validated schedules and executed commands | the panel renders `RESEARCH_PROPOSED_VS_EXECUTED_NOTE` (the P7.25 §20.2 note, extended); every proposal article carries the `[PROPOSAL — PROPOSED; never executed]` heading; the closed proposal vocabulary carries ONLY `PROPOSED` — there is no executed state this surface can relay (§20.2); validated schedules and executed commands live in the Schedules/Dispatch workspaces and are never merged into this section |
| 3 — safety-filter statistics and infeasibility outcomes for MARL; residual/termination status for ADMM | the two `NO_RUN` shadow-comparison rows name their statistics slots verbatim (MARL filter statistics: activation/infeasibility/refusal rates + the outcome of EVERY infeasibility; ADMM: r vs ε_pri, s vs ε_dual, iteration count) with `statistics_note: None` — an empty denominator is NEVER a rate of zero (the P8.15 `EVAL_LOG_EMPTY` refusal mirrored at the surface) |
| 4 — explicit confirmation for any promotion action; record that promotion requires the protected gates and per-feature qualification | the promotion-REQUEST affordance requires the labelled `aria-required` checkbox before the button enables; the request is RECORDED LOCALLY ONLY (no endpoint exists); `PROMOTION_REQUEST_NOTE` states the protected validation → feasibility → authorization gates (ADR-010) and per-feature qualification (P8.29); a research proposal can never change the control-authority mode |
| 5 — the negative test: promote without the gates → refused and audited; the console cannot issue/authorize/fast-track | the structural refusal is the server surface: NO promote/issue/authorize/fast-track/approve endpoint exists anywhere on the research surface (verified against routes_research.rs/openapi.rs — the DTO vocabulary carries no such state to relay). The UI walkthrough record is UNRUN (§7) |

### 2.4 The data source — the honest seam (recorded)

The control plane holds NO research run: the shipped default
`RecordedResearchStatus` serves the RECORDED Phase 8 status (the module
registry with its closed P8.20 vocabulary, the recorded OPEN blockers —
the torch/PyG and RL-framework DECISION-POINTs, the unsigned CBF
derivation, the missing P8.35 hold-out manifest, ANDES never installed,
the baseline RESULTS artifact not frozen — and the mode lock), recorded
verbatim from the phase decision records on 2026-11-14. Every RUN fact
(proposals, comparisons, statistics) is an explicit gap. A drifted record
and a drifted row are a review failure, not a runtime hazard.

## 3. Acceptance criteria — honest status

| Ledger acceptance criterion | Status |
|---|---|
| An operator can distinguish a research proposal from a validated schedule and from an executed command | THE STRUCTURE IS IN CODE (the `PROPOSED`-only vocabulary, the labelling note, the mode lock); the operator walkthrough that demonstrates it is **UNRUN** (§7) |
| Provenance and validation outcome visible per proposal | IN CODE (the provenance triple per `ResearchProposalDto`; the `VALIDATION_OUTCOME_MISSING` refusal); no proposal exists to display — the empty state is honest |
| No research output is displayed as executed | IN CODE (the closed vocabulary cannot express "executed"; the note renders); verified by the written tests, which are UNRUN |
| A promotion attempt that skips a protected gate is refused and audited | THE REFUSAL IS STRUCTURAL (no promotion endpoint exists; the local-request affordance performs no promotion); the step-5 walkthrough (attempt + confirm refusal + confirm auditability) is **UNRUN** |

## 4. Contract gaps → Phase 7 follow-ups (recorded, not invented)

1. **The closed-enum action id `research.status.read` does not exist**
   (carried from the P7.25 `plugin.inventory.read` gap): the route
   evaluates `telemetry.read` as the interim deny-by-default gate; the
   single swap point is the call site. Extending the closed `Action`
   vocabulary is flexcore-authz's decision — never invented here.
2. **No research run exists, so no proposal/comparison/statistic can be
   served** — not a gap fix but the recorded state: the surface is the
   seam a future evaluation forwarder composes into
   (`AppState::with_research_status`), exactly the P7.25
   `with_plugins_inventory` pattern.
3. **The MARL shadow-log schema carries no fallback field** (carried from
   `15_marl_evaluation.md` §5): when a composed source ever serves filter
   statistics, the fallback facts render `NOT_RECORDED_IN_LOG_SCHEMA`
   rather than a fabricated value; the additive log-schema extension is
   the recorded follow-up.
4. **The generated-client drift test lands with the first install** (the
   api/plugins.ts note carried forward): the hand-written wire mirrors in
   `api/research.ts` are the stand-in until npm dependencies exist.

## 5. Non-claims (restated)

Nothing here is built, executed, or verified by tooling: the TypeScript
was never compiled (no tsc/vite ran), the vitest suites never ran, the
Rust was never compiled (no cargo command ran). No research proposal,
shadow comparison, filter statistic, or residual summary exists — the
surface serves the EMPTY recorded state, never a plausible one. No
research output is or can be displayed as executed. The mode is locked
to `simulation-shadow` and no proposal/profile/plugin can raise it. A
promotion requires the protected validation → feasibility →
authorization gates and, for any field use, per-feature qualification
(P8.29) — none of which starts on this console. The entire surface is
simulation-only reporting (AGENTS.md rule 5; ADR-010).

## 6. Reviewer-verification procedure (the human gate)

```sh
# (a) the documented-tools gate:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the console build/test pass (the human gate; UNRUN — the loop bans
#     npm install/build/test):
#     cd console && npm install && npm run lint && npx tsc -b && \
#       npx vitest run src/workspaces/ResearchWorkspace.test.tsx
#     expected: the written suite passes (states distinguishable,
#     vocabularies verbatim, GET-only client, confirmation-gated
#     request, no fabricated rate).

# (c) the control-plane compile/test pass (UNRUN — no cargo command ran):
#     cargo test -p flexcore-control-plane
#     expected: the routes_research unit tests pass (the closed module
#     vocabulary, zero run facts, the X-5 dynamic provenance, the
#     proposal-side lifecycle, the surface note).

# (d) the display-only structural check (by-citation seam):
grep -rn "promote\|authorize\|fast-track\|issue" \
  console/src/api/research.ts console/src/workspaces/researchWorkspaceData.ts
#     every hit must sit inside a NOTE/prohibition string or a doc
#     comment — no callable may carry a promotion/authorization path.

# (e) the ledger's verification method — the OPERATOR WALKTHROUGH
#     (UNRUN, human gate): against the lab, confirm each state is
#     distinguishable: a research proposal (labelled PROPOSAL, never
#     executed) vs a validated schedule (Schedules workspace) vs an
#     executed command (Dispatch workspace); provenance + validation
#     outcome visible per proposal; the step-5 negative check (attempt a
#     gate-skipping promotion — the console offers no such path; confirm
#     the refusal is structural and the request affordance records only
#     a LOCAL event).

# A green (a)-(d) earns only a one-line "human build/test gate: PASSED
# (date)" appended here; failures are recorded verbatim and the ledger
# tick reverts to `- [ ]`.
```

## 7. UNRUN register (the human gate inputs)

| Item | Status |
|---|---|
| npm install / lint / tsc / vitest (the console pass) | UNRUN — loop bans |
| cargo build/test (the control-plane pass) | UNRUN — loop bans |
| the operator walkthrough (the ledger's verification method) | UNRUN |
| the step-5 negative test walkthrough (promotion refused + audited) | UNRUN |
| the green-run tick evidence | none — the tick records "implemented, awaiting human build/test" per ENTRY_NOTE |
