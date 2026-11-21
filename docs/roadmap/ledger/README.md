# FlexCore Roadmap Ledger — F-16 / F-17 — README

## What this is

The implementation ledger for the two proposed FlexCore roadmap
extensions: **F-16 Facility DER Assessment & Flexibility
Qualification** (PROPOSED post-v1 extension inside FlexCore) and
**F-17 Residential Solar & Storage Feasibility Planner** (RESEARCH
candidate as a separate companion product). It turns the principal
engineering system design into an end-to-end, checkable work plan:
7 waves, **76 subtasks**, each with testable acceptance criteria, so
both features can be implemented without missing anything and without
guessing.

- **Start here:** `MASTER_LEDGER.md` — the master tracker: all waves,
  design trace, entry/exit gates, dependencies, §16 gate mapping,
  §15 ADR mapping, status dashboard, definition of done, cross-wave
  risks.
- **Then:** the wave directory for the wave you are about to start.
  Each wave directory is self-contained for its implementing agent.

This ledger is a **PLAN**. All 76 subtasks are unchecked. Nothing is
implemented, built, tested, or run. Nothing is committed or pushed.
It was independently reviewed in iteration 01 (20 findings, all
patched) and re-reviewed in iteration 02 with zero findings remaining,
then adversarially audited in iteration 03 (45 findings, all patched)
and re-reviewed in iteration 04 (12 findings, all patched) and
re-reviewed in iteration 05 (7 findings, all patched) and re-reviewed
in iteration 06 (6 findings, all patched) and re-reviewed
in iteration 07 (4 findings, all patched) and re-reviewed
in iteration 08 (3 findings, all patched) and re-reviewed
in iteration 09 (1 finding, patched) and re-reviewed
in iteration 10 (1 finding, patched) and re-reviewed
in iteration 11 (zero findings — FINALIZED stamp restored;
see `FINALIZATION.md` and `_iterations/`). "Finalized" describes the
plan only — all 76 subtasks are unchecked and no implementation work
is done.

## Layout

```text
docs/roadmap/ledger/
  README.md                 — this file
  MASTER_LEDGER.md          — master tracker and status dashboard
  FINALIZATION.md           — verification record (finalized as a plan; see review history)
  wave-00-gate-0/                   Wave 0  = Gate 0 (design §14)
  wave-01-f16-phase-1/              Wave 1  = F-16 Phase 1 (design §3–§7)
  wave-02-f16-phase-2/              Wave 2  = F-16 Phase 2 (design §5.4–§5.6)
  wave-03-f16-phase-3/              Wave 3  = F-16 Phase 3 (design §7.3)
  wave-04-f17-phase-0/              Wave 4  = F-17 Phase 0 (design §9.1, §10)
  wave-05-f17-phase-1/              Wave 5  = F-17 Phase 1 (design §8–§11)
  wave-06-f17-later-phases/         Wave 6  = F-17 later phases (design §12)
```

Each wave directory contains one subtask file per subtask
(`01_*.md` … `NN_*.md`; file number matches subtask number). Each
subtask file has: ID (`W<w>.<nn>`), workstream, objective, design
trace (§), inputs, steps, deliverable, testable acceptance criteria
(checkboxes `- [ ]`), owner role, dependencies, verification method,
risks. Where a subtask owns a §15 ADR or a §16 gate, the file states
it explicitly ("Owns ADR-F16-01", "Owns §16 gate G3").

## Status conventions

- `- [ ]` = not started / not done. `- [x]` = done **only** when the
  acceptance criteria are evidenced in the deliverable the subtask
  names. Verbal agreement is not evidence. Never tick a box without
  evidence.
- Subtask IDs are stable: `W<w>.<nn>` (e.g. `W1.05`). New subtasks
  added in review continue the sequence; never renumber existing IDs,
  because other waves cross-reference them.
- "Workstream: FlexCore control-plane work" (Waves 0–3) vs
  "Workstream: companion-repo work (separate identity/database — NOT
  FlexCore control-plane work)" (Waves 4–6) is stated in every subtask
  file. Do not move work across that boundary.

## How to update the ledger

1. Work one wave at a time, in wave order within each feature
   (0 → 1 → 2 → 3 for F-16; 0 → 4 → 5 → 6 for F-17). Do not start a
   wave whose entry gate (see `MASTER_LEDGER.md`) is not met. Wave 4
   enters from Gate 0, not from Wave 3.
2. Before starting a wave, re-read the design sections it traces to,
   then re-verify the design's version/license/standard claims against
   the primary sources — versions move; the ledger's cut-off is
   21 Nov 2026. If a version moved, record the change; do not
   silently pin a different version.
3. Complete subtasks in ID order unless a subtask's Dependencies say
   otherwise. Tick the checkbox only with evidence attached. Twelve
   subtasks depend on a later-numbered subtask in the same wave
   (W0.08→W0.09, W0.08→W0.10, W1.10→W1.20, W1.18→W1.19, W1.18→W1.20,
   W4.04→W4.07, W4.04→W4.08, W4.04→W4.09, W4.05→W4.08, W5.10→W5.11,
   W5.10→W5.12, W6.05→W6.06); all twelve are acyclic — work the
   dependency first (see MASTER_LEDGER.md §4). Dependencies list
   prerequisites only (upstream inputs a subtask consumes). A subtask
   ID named in Inputs, steps or acceptance that is a downstream
   consumer, a test-oracle, or an affected subtask evidencing this
   subtask's output is a forward cross-reference, not a prerequisite,
   and must NOT be added to Dependencies — adding it would create a
   cycle (for example W0.09 is consumed by W1.11, W1.20 is consumed by
   W1.10, W5.03 is consumed by W5.12, W6.01 is consumed by W6.07).
   W4.07 step 1 documents this pattern for its own case (the W4.04
   prototype is a later verification consumer of the tier definitions,
   not a prerequisite for them).
4. When a wave's exit gate is met, complete its exit-gate review
   subtask with evidence per item, then hand over to the next wave
   using that subtask. (For F-16 Phase 3 and F-17 Phase 1 there is no
   design §14 exit gate — complete the W3.05 / W5.10 scope review
   instead.)
5. If you find a gap, an error, or a contradiction: do NOT silently
   fix the design's intent — fix the subtask, and record the finding
   for the independent review. If the design doc itself is wrong,
   record it; the design doc is edited only by its owners.
6. Decisions G1–G9 remain OPEN (design §16). A recommended default
   used for simulation or prototype work must never be written up as
   decided, approved, or committed. When a decision is made, write an
   ADR and update the ledger.

## Traceability rules

- Every subtask traces to a design-doc section (`§N` or `§N.M`). If a
  trace does not resolve to a real design heading, the subtask is
  wrong, not the trace format.
- If a subtask contradicts the design doc, fix the subtask. The
  foundations document (`docs/foundations/`) wins over the report on
  any conflict (AGENTS.md §2); the design doc is built from it.
- No duplicated subtasks across waves: cross-reference the owning
  `W<w>.<nn>` ID instead.
- Canonical units everywhere (FlexCore boundary): MW, MVAr, MVA, MWh.
  `P > 0` = export at the point of connection. SOC is a fraction in
  [0,1] at the API boundary. Telemetry quality is exactly one of
  `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE`.
- Solver discipline: HiGHS is LP/QP/MIP, not SOCP. Clarabel is convex
  conic, not MISOCP. AC power flow is not AC-OPF. IPOPT is local, not
  a global-optimality guarantee.

## Naming discipline — the three audit/M&V concepts (never conflate)

- **"energy audit"** = the F-16 assessment domain (facility energy
  assessment and flexibility qualification). This is the only sense in
  which the phrase is used in this ledger.
- **Operational append-only audit log** (flexcore-audit, P1.10) =
  operational evidence of what the platform did. Do not rename or
  repurpose it as the energy-audit domain.
- **DR event M&V** (P16.05) = demand-response program baselines and
  event delivery measurement.
- **Efficiency savings M&V** (W3.01/W3.02, IPMVP) = post-retrofit
  efficiency savings verification. Separate from DR event M&V.

## Control authority — read this before any wave

- The entire 0.x line is **simulation-only**: no writes to physical
  devices. **v1.0.0 defaults to operator-approved control.** Bounded
  automation is post-1.0, per deployment, site, and asset class.
- **F-16 qualification is not dispatch authority.** A qualified
  candidate, an onboarding draft, and even an approved Core asset do
  not grant dispatch authority; dispatch remains governed by the Core
  command lifecycle and control-authority modes.
- **F-17 never receives dispatch permission.** The household planner
  has no path to FlexCore dispatch, commands, credentials, or
  site-edge connections. The only F-16/F-17 exchange is the governed
  §12 enrollment handoff (W6.05/W6.06).
- FlexCore is not certified, not a safety case, and not permission for
  live grid control. Never write text that implies otherwise. It is a
  supervisory orchestration layer: no primary frequency response,
  synthetic inertia, relay protection, inverter inner-loop control, or
  sub-cycle control. ML/RL outputs are proposals only and cannot
  directly dispatch.

## What is NOT in this ledger

- The **generic household energy audit** is explicitly deprioritized
  (foundations.md §5) and is **not** in this ledger. Do not add it.
  F-17 is a solar-and-storage feasibility planner, not a whole-house
  audit.
- Neither feature delays the v1.0.0 gates (foundations.md §8). Both
  are post-v1. F-17 stays a research candidate until its Phase 0
  prototype demonstrates real demand.

## Sources of truth (in order)

1. `docs/foundations/Neryva_FlexCore_System_Design_Specification_v1.0.pdf` / `.docx`
2. `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md`
3. `docs/roadmap/foundations.md` (roadmap research: F-16 PROPOSED,
   F-17 RESEARCH candidate)
4. `docs/roadmap/flexcore_extensions_principal_engineering_design.md`
   (the system design this ledger is built from; research cut-off
   21 Nov 2026)
5. `ledger/` (the v1 implementation ledger — the plan this roadmap
   ledger extends, never authoritative over 1–4)
6. This ledger (implementation plan — never authoritative over 1–5)

This ledger adds files only under `docs/roadmap/ledger/`. It does not
edit `ledger/`, `docs/foundations/*`, `docs/roadmap/foundations.md`,
or the design doc, and does not rename or renumber design sections.
