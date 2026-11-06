# FlexCore Implementation Ledger — README

## What this is

The implementation ledger for **Neryva FlexCore** (report v1.1, research
cut-off 7 November 2026). It turns the product and architecture report into
an end-to-end, checkable work plan: 11 phases, **381 subtasks**, each with
testable acceptance criteria, so the platform can be implemented without
missing anything and without guessing.

- **Start here:** `MASTER_LEDGER.md` — the master tracker: all phases,
  release mapping, entry/exit gates, dependencies, D1–D6 gate mapping,
  status dashboard, definition of done, cross-phase risks.
- **Then:** the phase directory for the phase you are about to start.
  Each phase directory is self-contained for its implementing agent.

## Layout

```text
ledger/
  README.md                 — this file
  MASTER_LEDGER.md          — master tracker and status dashboard
  phase-00-product-freeze/          Phase 0  = Report Stage 0
  phase-01-rust-foundation/         Phase 1  = Stage 1
  phase-02-asset-telemetry/         Phase 2  = Stage 2
  phase-03-flexibility-scheduling/  Phase 3  = Stage 3
  phase-04-grid-feasibility-dispatch/   Phase 4 = Stage 4
  phase-05-operator-console-simulator/  Phase 5 = Stage 5
  phase-06-edge-protocol/           Phase 6  = Stage 6
  phase-07-plugin-sdk/              Phase 7  = Stage 7
  phase-08-advanced-coordination/   Phase 8  = Stage 8
  phase-09-pilot-qualification/     Phase 9  = Stage 9
  phase-10-v1-release-hardening/    Phase 10 = v1.0.0 release gate (no report stage)
  _iterations/              — review-iteration audit trail (see below)
```

Each phase directory contains:

| File | Purpose |
|---|---|
| `ledger_phase_<N>.md` | THE subtask ledger: step-by-step subtasks, each with ID, objective, report trace (§), inputs, steps, deliverable, testable acceptance criteria, owner role, dependencies, verification method, risks. Checkboxes `- [ ]`. |
| `PLAN.md` | Detailed implementation plan: scope/non-goals, decisions, components, data contracts, interfaces, sequencing, roles, relative effort, risks, what must NOT be built yet, how the §24 vertical slice lands here. |
| `CONTEXT.md` | Phase context for an implementing agent: background, report sections to re-read, units/sign conventions, terminology, constraints, prior-phase outputs consumed, D1–D6 relevance, open questions. |
| `RESEARCH.md` | Online-researched latest approaches for this phase, checked 7 November 2026 against primary sources, with version/status + source URL + date checked. Unverified items are labelled ASSUMPTION or recorded as open verification tasks. |
| `CHECKLIST.md` | Exit-gate checklist mirroring the phase's §23 exit gate, each item with the evidence required. |

## Status conventions

- `- [ ]` = not started / not done. `- [x]` = done **only** when the
  acceptance criteria are evidenced in the deliverable the subtask names.
  Verbal agreement is not evidence. Never tick a box without evidence.
- Subtask IDs are stable: `P<N>.<nn>` (e.g. `P4.33`). New subtasks added
  in review continue the sequence; never renumber existing IDs, because
  other phases cross-reference them.
- Severity in iteration findings: **Critical** = would make a release
  decision unsafe; **High** = a claim a subtask would act on, or a missing
  proof for a gate; **Medium** = stale detail a re-verify rule would catch;
  **Low** = precision polish or an already-flagged open verification.

## How to update the ledger

1. Work one phase at a time, in phase order (0 → 10). Do not start a
   phase whose entry gate (see `MASTER_LEDGER.md`) is not met.
2. Before starting a phase, re-read its `CONTEXT.md`, `PLAN.md`, and
   `RESEARCH.md`, then re-verify the RESEARCH.md versions against the
   primary sources — versions move; the ledger's cut-off is 7 Nov 2026.
   If a version moved, update RESEARCH.md and record the change; do not
   silently pin a different version.
3. Complete subtasks in ID order unless a subtask's Dependencies say
   otherwise. Tick the checkbox only with evidence attached.
4. When a phase's exit gate is met, complete its `CHECKLIST.md` with
   evidence per item, then hand over to the next phase using the handover
   subtask (the last subtask of each phase).
5. If you find a gap, an error, or a contradiction: do NOT silently fix
   the report's intent — fix the subtask, and record the finding in a new
   `_iterations/iteration-NN/findings.md` (see below). If the report
   itself is wrong, record it; the report is edited only by its owners.
6. Decisions D1–D6 remain OPEN (`.agent/DECISIONS.md`). A recommended
   default used for simulation work must never be written up as decided,
   approved, or committed. When a decision is made, write an ADR
   (`.agent/skills/adr-authoring/`) and update the ledger.

## Traceability rules

- Every subtask traces to a report section (`§N` or `§N.M`) and, where it
  applies, to foundations §6. If a trace does not resolve to a real
  report heading, the subtask is wrong, not the trace format.
- If a subtask contradicts the report, fix the subtask. The foundations
  document wins over the report on any conflict (AGENTS.md §2).
- No duplicated subtasks across phases: cross-reference the owning
  subtask ID instead.
- Canonical units everywhere: MW, MVAr, MVA, MWh. `P > 0` = export at
  the point of connection. SOC is a fraction in [0,1] at the API boundary.
  Telemetry quality is exactly one of
  `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE`.
- Solver discipline (AGENTS.md §3): HiGHS is LP/QP/MIP, not SOCP.
  Clarabel is convex conic, not MISOCP. AC power flow is not AC-OPF.
  IPOPT is local, not a global-optimality guarantee.

## Control authority — read this before any phase

- The entire 0.x line is **simulation-only**: no writes to physical
  devices. v0.3.0 adds simulation/shadow. v0.4.0 is read-only / test-lab
  by default. **v1.0.0 defaults to operator-approved control.**
  Bounded automation is post-1.0, per deployment, site, and asset class.
- FlexCore is not certified, not a safety case, and not permission for
  live grid control. Never write text that implies otherwise. It is a
  supervisory orchestration layer: no primary frequency response,
  synthetic inertia, relay protection, inverter inner-loop control, or
  sub-cycle control. ML/RL outputs are proposals only and cannot
  directly dispatch (ADR-010).

## Iterations (audit trail)

The ledger was built in one pass and then reviewed in **10 iterations**
(`_iterations/iteration-01/` … `iteration-10/`), each with a
`findings.md` recording missing subtasks, issues, suggestions, stale
research, severity, the exact file + subtask ID each item patches, and
an applied-vs-deferred log. `_iterations/ITERATION_LOG.md` summarises
them. The final assembly pass applied the two Medium corrections the
iteration-09 reviewer had to defer (out of its patch scope); no
Critical item was ever found, and every High item found now has an
owning subtask — they are open *work*, not unrecorded gaps.

## Sources of truth (in order)

1. `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx`
2. `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
3. `.agent/*.md` (digests — never authoritative on their own)
4. This ledger (implementation plan — never authoritative over 1–3)

This ledger adds files only. It does not edit `docs/foundations/*`,
does not rename or renumber report sections, and does not edit the
report itself.
