# P8.29 — Per-feature qualification record: what field use would require, and why none of it is granted here

Date: 2026-11-14. Subtask: P8.29 (`ledger_phase_8.md` §P8.29). Owner
role: Research lead + product. Report trace: §2.1 (v1.x/v2.x
per-feature qualification); §22.3; §23 Stage 9. Inputs: P8.23
(`23_cross_module_comparison.md`), P8.26 (benchmark paper — concurrent
wave 6b, cited as the phase-report deliverable, not as evidence here),
the P8.02 scope freeze (`02_research_scope_freeze.md` — prohibitions
X-1/X-2/X-4/X-9), ENTRY_NOTE.md boundary 1, the P8.13 MARL design
(`13_marl_design.md`), the Phase 7 protection patterns this record
names as preconditions (`crates/flexcore-kernel/src/{tenant_scope.rs,
plugin_path_protection.rs, revocation.rs, signing.rs}`,
`crates/flexcore-research/src/{tenant_isolation.rs,
proposal_windows.rs}`), CHECKLIST.md.

## 1. What this record is — the absence of qualification is a RECORDED FACT

This record states, per research module, **what field use would
require** — and records explicitly, per module and per requirement row,
that **this phase grants none of it**. Every requirement row below
carries a `status` column filled NOW with `NOT GRANTED / NOT EVIDENCED`
(the honest state as of the date above). There is no "in progress"
row, no "planned" row, and no row whose evidence exists in any form:
**the qualification lists in this record are requirements, never a
checklist already half-done** (the ledger's recorded risk).

The record exists so that the absence of qualification is a **RECORDED
FACT, not an oversight** — so that no later phase can grant field use
"by implication" because nobody wrote down what qualification would
have taken (the ledger's recorded risk, verbatim substance). Routing:
this record is **routed to product on 2026-11-14** for the
per-feature qualification decision (§2.1: a v1.x/v2.x per-feature
decision). The decision itself is a LATER decision, made by product,
per feature, with the evidence classes of §2 — not this phase's, and
not this record's.

**Every evaluation claim in this phase is per-feature-qualified
NOWHERE** (ENTRY_NOTE.md boundary 1, verbatim substance). The ceiling
of Phase 8 is **simulation and shadow** (§2.1 v2.x posture; the Phase 8
gate exception, ENTRY_NOTE.md). Nothing in this record is a promise,
schedule, or intention of field use; §5 states what field use WOULD
require in the conditional mood only.

## 2. The evidence classes field use would require (the qualification vocabulary)

These are the classes a per-feature qualification decision would draw
on (§22.3; §23 Stage 9; §5.7/§9.1 plugin protections). Naming a class
here grants nothing; each module's table in §4 rows these classes with
status `NOT GRANTED / NOT EVIDENCED`.

| Class | What it is | What would close it |
|---|---|---|
| Site-specific validation evidence | Validation of the module's output against the SPECIFIC site's model, devices, and constraints | A witnessed validation run against the site model, reviewed by a power-systems reviewer not involved in the module's development — EMPTY cell now |
| Independent re-validation evidence | The P8.05 harness (and its §12.5 feasibility gate) passing the module's outputs on the deployment scope | Harness results per module, harness version stamped, reviewer-owned — EMPTY cell now (the harness exists WRITTEN-UNCOMPILED; no run) |
| HIL (hardware-in-the-loop) test evidence | The module's proposals exercised against real controller hardware in a lab | A witnessed HIL campaign record with the campaign scope — EMPTY cell now |
| Dynamic-stability evidence | For anything transient: ANDES-class time-domain studies of the proposals under the recorded disturbance set (§19.1) | A dynamic evaluation with model/case references, reviewed — EMPTY cell now (P8.17 template exists with EMPTY cells) |
| Cyber review | A security review of the module, its worker, its artifacts, and its data path | The P8.25-class review PLUS deployment-scope review, findings closed — EMPTY cell now (P8.25 is a written review, not a deployment-scope one) |
| Commissioning | Site commissioning under operator supervision | A commissioning record signed by the operator — EMPTY cell now |
| Market / interconnection qualification | Where applicable: the market program's or interconnection agreement's own qualification process | The program's qualification evidence — EMPTY cell now |
| Operator approval + mode change | The control-authority mode moved beyond simulation/shadow, per deployment, per site, per asset class (§5.7; AGENTS.md §4 release ladder) | A recorded operator decision + the mode-change record — EMPTY cell now |
| Plugin protections (§5.7/§9.1) | The Phase 7 plugin protections holding for the research worker as a plugin: signing (P7.14), tenant scope (P7.39), revocation (P7.38), the protected validation → feasibility → authorization path | The protections verified FOR THE DEPLOYED WORKER — the patterns exist in code (kernel; the research mirror in `tenant_isolation.rs`), the deployment evidence does not — NOT EVIDENCED |
| Stage 9 gate (§23 Stage 9) | The v1.0.0 pilot-ready gate the report defines | The Stage 9 exit-gate record — does not exist; EMPTY cell now |

## 3. The binding statements (verbatim, pinned by reviewer check)

The two boundary statements this record must carry verbatim (the
task's requirement; the reviewer checks §7 confirms they are present
and unmodified):

> "No live device commands from MARL — a research MARL worker has no
> code path to the dispatch gateway; this is prohibited, not deferred"
> (`02_research_scope_freeze.md` X-2).

> "no research module has any field authority; a MARL worker issuing
> live device commands is PROHIBITED, not deferred" (`ENTRY_NOTE.md`
> boundary 1).

These are **PROHIBITIONS, not deferrals**: no qualification pathway in
this record, and no later decision routed to product, can convert them
into a deferred feature. The enforcement points are in code (the P8.14
boundary tests; the P7.31 nine code-level enforcement points; the
P8.04 proposal-only boundary) — not only in this prose.

## 4. The per-module qualification records

Status key for every row: **`NOT GRANTED / NOT EVIDENCED`** means (a)
no qualification decision grants the row, and (b) no evidence for the
row exists in this repository or anywhere else. There are no other
status values in this record — a row would only ever change when the
evidence exists AND product records the decision, in a NEW version of
this record.

### 4.1 ADMM/DMPC coordination provider (P8.07–P8.09)

| # | Requirement (field use would need) | Evidence that would close it | status |
|---|---|---|---|
| A1 | Site-specific validation of the coordination schedules against the site's coupled-feeder model | Witnessed validation run + independent reviewer sign-off — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A2 | Independent re-validation (P8.05 harness + §12.5 feasibility gate) on the deployment scope | Harness results per module, stamped with `HARNESS_VERSION` — EMPTY cell (harness WRITTEN-UNCOMPILED; never run) | NOT GRANTED / NOT EVIDENCED |
| A3 | HIL test of the schedule dispatch path | Witnessed HIL campaign record — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A4 | Cyber review of the worker and its data path | Deployment-scope review with findings closed — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A5 | Commissioning under operator supervision | Commissioning record signed by the operator — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A6 | Market/interconnection qualification where the program requires it | The program's own qualification evidence — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A7 | Operator approval + control-authority mode change (per deployment/site/asset class) | Recorded operator decision + mode-change record — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A8 | §5.7/§9.1 plugin protections verified for the deployed worker | Signing/tenant-scope/revocation/protected-path evidence for the deployed worker — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| A9 | Stage 9 gate (§23 Stage 9) | The Stage 9 exit-gate record — does not exist | NOT GRANTED / NOT EVIDENCED |

**Module statement: the ADMM/DMPC provider is NOT qualified for field
use. Simulation is its ceiling in this phase.** (No optimality claim
beyond the stated convexity assumptions applies anywhere — X-3,
§13.2; AGENTS.md rule 4.)

### 4.2 GNN warm-start / screening (P8.10–P8.12)

| # | Requirement | Evidence that would close it | status |
|---|---|---|---|
| G1 | Site-specific validation: the warm start/screening shown sound on the SPECIFIC site's topologies, within the tested range (X-7: no generalization claim beyond the tested range) | Witnessed validation on the site's topology set — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| G2 | Independent re-validation (P8.05 harness + §12.5 gate): every GNN output passing the same physical validation as any other candidate (§13.1) | Harness results — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| G3 | Model-artifact signing + verification at load (P8.32) | The P8.32 tamper/unsigned/revoked refusal record — P8.32 is a CONCURRENT wave-7b subtask; even when written, it is WRITTEN-UNRUN — NOT EVIDENCED | NOT GRANTED / NOT EVIDENCED |
| G4 | Training-data provenance and poisoning controls (P8.35) | The P8.35 inventory + quarantine record — EMPTY measured cells | NOT GRANTED / NOT EVIDENCED |
| G5 | HIL / cyber / commissioning rows (A3–A5 shape) | EMPTY cells | NOT GRANTED / NOT EVIDENCED |
| G6 | Cross-tenant model-serving treatment: the recorded approval path (`tenant_isolation.rs`) AND an explicit weight-leakage treatment (the residual risk recorded in `31_tenant_isolation.md` §3 — approval does not treat leakage) | Approval decision record + leakage treatment study — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| G7 | Operator approval + mode change + Stage 9 gate | EMPTY cells (A7/A9 shape) | NOT GRANTED / NOT EVIDENCED |

**Module statement: the GNN warm-start/screening worker is NOT
qualified for field use. Simulation/shadow is its ceiling in this
phase.**

### 4.3 Safe MARL (simulation/shadow) (P8.13–P8.15)

| # | Requirement | Evidence that would close it | status |
|---|---|---|---|
| M1 | The CBF/safety-layer derivation signed by a power-systems reviewer (model + relative degree composed through the power-flow map) | The signed derivation — **NOT YET DERIVED; training BLOCKED** (`13_marl_design.md` §3 — the recorded gap) | NOT GRANTED / NOT EVIDENCED |
| M2 | Site-specific validation of the policy + filter under the site's regimes | Witnessed validation — EMPTY cell | NOT GRANTED / NOT EVIDENCED |
| M3 | Dynamic-stability evidence for anything transient (the proposals applied in shadow to the simulated system, model/case-referenced, §19.1) | P8.17-class dynamic evaluation with EMPTY cells — nothing measured | NOT GRANTED / NOT EVIDENCED |
| M4 | HIL / cyber / commissioning rows (A3–A5 shape) | EMPTY cells | NOT GRANTED / NOT EVIDENCED |
| M5 | Operator approval + mode change + Stage 9 gate | EMPTY cells | NOT GRANTED / NOT EVIDENCED |
| M6 | **The live-command boundary**: no code path from the MARL worker to the dispatch gateway — PROHIBITED, not deferred (§3 verbatim statements). No qualification pathway exists for this row at all | The P8.14 boundary tests (introspection + AST) remain the enforcement; there is NO evidence class that "closes" this row, because it is not a gap — it is a prohibition | **PROHIBITED — NOT GRANTED, NOT EVIDENCED, AND NOT QUALIFIABLE BY THIS RECORD** |

**Module statement: the MARL worker is NOT qualified for field use.
Simulation/shadow is its ceiling in this phase; the live-command
boundary is a prohibition, not a qualification item.** (Zero observed
violations from a finite test set is a test outcome, not a guarantee —
X-4.)

### 4.4 ANDES dynamic integration (P8.16–P8.17)

| # | Requirement | Evidence that would close it | status |
|---|---|---|---|
| D1 | Dynamic-stability evidence produced, model/case-referenced, and reviewed (the §19.1 dynamic track) | P8.17 template with EMPTY cells — nothing measured | NOT GRANTED / NOT EVIDENCED |
| D2 | The steady-state/transient boundary held (X-5: a steady-state power-flow tool is not transient-stability evidence — and conversely) | Boundary statement reviewed — the P8.16 statement exists as prose; no qualification content | NOT GRANTED / NOT EVIDENCED |
| D3 | ANDES pin verified from a primary source + GPLv3 isolation confirmed (gate G-4, legal) | The pin + legal confirmation — OPEN | NOT GRANTED / NOT EVIDENCED |
| D4 | Operator approval + mode change + Stage 9 gate (dynamic results are evidence about a DECLARED MODEL, never the real grid — ADR-012 posture) | EMPTY cells | NOT GRANTED / NOT EVIDENCED |

**Module statement: the ANDES dynamic integration is NOT qualified for
field use. It is a research instrument about declared models, not a
field capability.**

### 4.5 HELICS co-simulation (P8.18)

| # | Requirement | Evidence that would close it | status |
|---|---|---|---|
| H1 | Co-sim cross-checks (P8.39) run and reviewed | EMPTY cells | NOT GRANTED / NOT EVIDENCED |
| H2 | The HELICS-is-transport boundary held (X-6: federation results are not power-system results) | Boundary statement reviewed — prose only | NOT GRANTED / NOT EVIDENCED |
| H3 | HELICS pin verified + license consequence recorded | P8.03 §2 — pinned 3.7.0 from Phase 5; consequence recording is the P8.35 dataset/license record — EMPTY cells | NOT GRANTED / NOT EVIDENCED |
| H4 | Operator approval + mode change + Stage 9 gate | EMPTY cells | NOT GRANTED / NOT EVIDENCED |

**Module statement: the HELICS co-simulation line is NOT qualified for
field use.**

### 4.6 Portfolio analytics (reporting-only) (P8.22)

| # | Requirement | Evidence that would close it | status |
|---|---|---|---|
| P1 | Reporting-only verification by inspection (analytics never influence dispatch) — re-verified for any deployment | The inspection record — P8.22's written verification exists; deployment-scope re-verification does not | NOT GRANTED / NOT EVIDENCED |
| P2 | Validation against the deterministic baseline's accounting within the stated tolerance, on the tested scenarios | EMPTY measured cells | NOT GRANTED / NOT EVIDENCED |
| P3 | Operator approval for any surfaced reporting workflow (the console surfacing is P8.27, concurrent wave; proposals shown AS proposals) | EMPTY cell | NOT GRANTED / NOT EVIDENCED |

**Module statement: portfolio analytics is NOT qualified for any field
or operator-workflow use beyond recorded reporting validation. It never
influences dispatch — that is a structural prohibition (P8.22), not a
qualification target.**

### 4.7 Benchmark/reproducibility infrastructure (P8.19–P8.20, P8.26) — not a proposal-producing module

This line produces no proposals and has no field-facing surface; field
use "qualification" does not apply. Recorded for completeness: the
reproducibility rerun test (P8.20) has EMPTY measured cells; the
benchmark paper (P8.26) is the concurrent wave's deliverable and is
cited, not evidenced, here. **status: N/A — no field surface; the
reproducibility claims are EMPTY-cell-limited.**

## 5. What this record does NOT do

- It does not grant, schedule, or intend any field use. The conditional
  mood of §2/§4 ("would require") is the whole of what this record says
  about field use.
- It does not close or alter any D-item (D1–D6 OPEN; the per-feature
  qualification decision is product's, later).
- It does not convert any §3 prohibition into a deferral: the MARL
  live-command boundary, the no-field-authority boundary (X-1), and the
  no-productizing boundary (X-9) stand as prohibitions.
- It does not qualify anything for the v0.x simulation-only releases —
  those need no qualification, and no claim here should be read as one.

## 6. Phase 7 follow-ups recorded (contract gaps, never invented fixes)

| # | Follow-up | Owner / activation point |
|---|---|---|
| QF-1 | The Phase 7 worker template and `worker_api.proto` carry no per-feature qualification metadata path; if a future qualified deployment needs qualification evidence to travel with worker results, that is a Phase 7 contract extension (recorded decision + buf governance), never a parallel contract | Phase 7 contract owners; only if product's per-feature qualification decision requires it |
| QF-2 | The Phase 7 release-ladder artifacts (AGENTS.md §4 table) have no per-feature qualification record slot for research features; when product makes the first qualification decision, the ladder needs a recorded amendment process | Product + docs owner; at the first qualification decision |

## 7. Reviewer verification procedure

1. Confirm §3's two verbatim statements are present and unmodified
   against `02_research_scope_freeze.md` X-2 and `ENTRY_NOTE.md`
   boundary 1.
2. For each module table in §4: confirm every row's status is exactly
   `NOT GRANTED / NOT EVIDENCED` (or the recorded N/A / PROHIBITED
   variants of §4.3 M6 and §4.7), and that NO row carries a date,
   ticket, or evidence link that implies progress.
3. Confirm no sentence in this record is future-tense about field use
   ("will be deployed", "is planned for") — a reviewer finding such a
   sentence is a finding AGAINST this record.
4. Confirm the routing date (2026-11-14) is present and that the record
   names the decision owner (product) without recording a decision.
5. Confirm the measured/EMPTY-cell discipline: no number in this record
   is presented as a result (there are none — this record contains no
   measured cells at all).
6. Trace each §2 evidence class to its cited source (§22.3, §23 Stage 9,
   §5.7/§9.1, §19.1).

## 8. Honest status

**WRITTEN-UNCOMPILED.** This record is documentation; there is no code
to compile for P8.29 itself. The modules it references exist
WRITTEN-UNCOMPILED (the P8.05 harness, P8.31 `tenant_isolation.rs`,
P8.34 `proposal_windows.rs` — no cargo command executed in this loop),
and every evaluation record it names (P8.09/P8.12/P8.15/P8.17/P8.23)
is a template with EMPTY cells. Nothing here is evidence that any
module ever ran, validated, or qualified. The ledger tick on P8.29
means *implemented (this record exists), awaiting human build/test*
— and, for the qualification content itself, awaiting nothing at all:
it is NOT GRANTED, and this record is how that fact stays true.
