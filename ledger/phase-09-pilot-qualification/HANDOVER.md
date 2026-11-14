# HANDOVER — Phase 9: Pilot Qualification

Status: **CLOSED** — 41/41 subtasks implemented (P9.01–P9.41); the §23
Stage 9 exit gate is **NOT MET** (prepared, not demonstrated — no customer,
site, or deployment exists; see `30_phase_exit_gate_record.md` §5). Loop
bans held throughout: nothing installed, nothing built, no tests run, no
CI run, no lab/HIL/drill/rehearsal/training/review/approval — everything
WRITTEN-UNRUN.

## Blockers carried (human gate inputs)

1. Stages 1–8 gates NOT HELD — all demonstrations/runs/sign-offs pending.
2. A pilot cannot be executed by agents — every site/commissioning/test/
   drill/training deliverable is a PLAN or TEMPLATE with EMPTY cells.
3. D1/D3/D5/D6 remain OPEN — agents record reviews and recommendations
   only (ENTRY_NOTE boundary 2).
4. All execution evidence (lab, HIL, field, drills, authority approvals)
   = human gate inputs, recorded UNRUN/UNSIGNED.

## Wave status

| Wave | Subtasks | Status |
|---|---|---|
| 1 | P9.01–P9.03 ∥ P9.31, P9.32 | done (unfrozen templates; edge rejection + audit proof code) |
| 2 | P9.04–P9.06 ∥ P9.33, P9.34 | done (D5/D6 reviews + closure PROPOSAL; enrollment + update code) |
| 3 | P9.07–P9.09 ∥ P9.35, P9.36 | done (security design set; invariant map + drill plan) |
| 4 | P9.10–P9.12 ∥ P9.37, P9.38 | done (D1-OPEN designs; conformance + replay templates) |
| 5 | P9.13–P9.15 ∥ P9.39, P9.40 | done (commissioning/lab/HIL plans; perf template + traceability map v1) |
| 6 | P9.16–P9.18 ∥ P9.19–P9.21 | done (FAT plan/execution/telemetry; dry run, drills, training) |
| 7 | P9.22–P9.24 ∥ P9.25–P9.27 | done (procedures/DR; observability, control scope, LR register v0.1-draft) |
| 8 | P9.28, P9.29 | done (acceptance review + authority pack templates) |
| — | P9.41 (recovered) | done (regression template; honest finding: NO baseline run exists anywhere) |
| 9 | P9.30 (coordinator) | done (`30_phase_exit_gate_record.md`; CHECKLIST 41/41 with evidence annotations; audit + fixes F-1..F-5 incl. recovering the missed P9.41) |

## What was implemented

Preparation for a pilot that does not exist yet — plans, templates,
procedures, and real edge-security code, every execution cell EMPTY:
42 decision records in this directory; the consolidated limitations
register `27_...md` (`pilot-limitations-register-v0.1-draft`, LR-001..
LR-211 + LR-212..LR-225 proposed, 42 carried pointers); the versioned
traceability artifact `hil-field-traceability-map-v1.json` (empty
matrices, frozen); edge-agent additions `pilot_rejection_proof.rs`
(1,013 lines), `audit_proof.rs` (541), `enrollment_ceremony.rs` (1,785),
`update_verification.rs` (1,319) + 4 in-module test modules + 4
integration suites — ALL WRITTEN-UNCOMPILED/UNRUN. One real security
finding honestly carried: the P6 `command_verify.rs` sequence-only check
admits a re-signed conflicting payload under a used idempotency id —
closed ADDITIVELY by the P9.31 `FileCommandIdBinding` journal.

## How to build and test it / Blockers / Decisions still needed / Assumptions / Known risks

**Build/test (the human's gate pass):** `cargo build --workspace` →
`cargo clippy --workspace --all-targets -- -D warnings` → `cargo test
--workspace` (incl. the four new edge-agent modules) → CI green run →
then the human gate chain: P9.01 freeze (requires a real customer/site),
D1/D3/D5/D6 signed closures, commissioning (P9.13), lab (P9.14), HIL
(P9.15), FAT shadow-first (P9.16–P9.18), dry run/drills/training
(P9.19–P9.21), conformance/replay/perf/regression (P9.37–P9.41) —
filling commands in each record.

**Blockers (human gate inputs):** Stages 1–9 all NOT MET/NOT HELD; no
customer/site/hardware/lab/rig/CA/IdP exists; D6 closure is a human
decision (review + proposal recorded); P9.23 tabletop UNHELD → P9.36
drill UNSCHEDULABLE; no baseline scenario-regression run exists anywhere.

**Decisions still needed:** D1, D3, D5, D6 (recommendations recorded,
agent-undecided); production crypto selection (CSPRNG/SHA-256 —
DEPLOYMENT REQUIREMENTS); update-bundle producer ownership.

**Assumptions (labelled):** clock-quality pilot-profile 100 ms/30 000 ms
values are DEFAULTS not measurements; ANDES/HELICS/pandapower pins
carried UNVERIFIED; no certification/real-time-performance claims anywhere.

**Known risks:** all new code uncompiled — first build may surface
integration defects; the full risk surface is `27_...md` (LR-001..LR-225)
plus the carried Phase 6/7/8 registers; Stage 9 re-entry conditions in
`30_phase_exit_gate_record.md` §5.
