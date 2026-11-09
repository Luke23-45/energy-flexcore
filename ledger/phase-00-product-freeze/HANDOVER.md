# HANDOVER — Phase 0: Product and Domain Freeze

Status: **closed (code written, nothing built or tested)** — see the sections
below. The batch plan is at the bottom of this file.

## What was implemented

All 30 subtasks (P0.01–P0.30) have their deliverables written. Subtask → files:

| Subtask | Deliverable(s) |
|---|---|
| P0.01 | `01_scope_and_non_goals.md` (§0.3 verbatim, §1.3 audiences, §1.6 boundary, §2.1 ladder, sign-off lines) |
| P0.02 | `02_first_buyer_use_cases.md` (buyer = recommendation pending D2; §1.5 disposition matrix; deferred list; portfolio sheet) |
| P0.03–P0.06, P0.28 | `03_decision_records.md` (D1–D6 scheduled; gate-table skeleton §7); `.agent/DECISIONS.md` rows updated to SCHEDULED |
| P0.07 | `07_units_sign_conventions.md` + `crates/flexcore-domain/src/units.rs` (+ tests) |
| P0.08 | `08_telemetry_quality_vocabulary.md` + `crates/flexcore-domain/src/quality.rs` (+ tests) |
| P0.09 | `09_time_model.md` + `crates/flexcore-domain/src/time_model.rs` (+ tests) |
| P0.10 | `10_asset_capability_vocabulary.md` |
| P0.11 | `11_command_state_machine.md` (27 §14.1 edges, mechanically diffed) + `crates/flexcore-domain/src/command_state.rs` (+ tests incl. full frozen-edge table) |
| P0.12 | `12_grid_model_contract.md` |
| P0.13 | `13_solver_class_discipline.md` |
| P0.14 | `14_event_vocabulary.md` (26 events) + `crates/flexcore-domain/src/events.rs` (+ tests) |
| P0.15 | `15_api_surface.md` (16 resources) + `crates/flexcore-domain/src/api_paths.rs` (+ tests) |
| P0.16 | `16_tenancy_storage_principles.md` |
| P0.17 | `17_threat_model.md` |
| P0.18 | `18_control_authority.md` |
| P0.19 | `19_plugin_model.md` |
| P0.20 | `20_deployment_build_form.md` |
| P0.21 | `docs/architecture/adr/ADR-001.md` … `ADR-012.md` (status "Proposed"; none claims a D1–D6 decision) |
| P0.22 | `22_console_scope.md` |
| P0.23 | `23_simulation_test_strategy.md` |
| P0.24 | `24_vertical_slice_map.md` |
| P0.25 | Evidence already complete in `RESEARCH.md` §5 (check date = the cut-off date, 2026-11-07) |
| P0.26 | `26_foundations_crosscheck.md` (44-row table; **zero unresolved conflicts**; both known divergences recorded as compatible refinements) |
| P0.27 | `27_handover_pack.md` (P0.01–P0.26 index with location + status; open questions with owning gates; Phase 1 entry condition) |
| P0.29 | `29_exit_gate_review.md` — outcome recorded as **NOT HELD** (see Blockers) |
| P0.30 | `30_anti_scope_audit.md` (negative list + the recorded code-contracts deviation) |

Workspace/manifests: root `Cargo.toml` (workspace, members =
`crates/flexcore-domain`, resolver 2, edition 2021),
`crates/flexcore-domain/Cargo.toml` (no dependencies, no dev-dependencies),
`crates/flexcore-domain/src/{lib,units,quality,time_model,command_state,events,api_paths}.rs`
— every file headed with its subtask ID; every module carries inline
`#[cfg(test)]` tests derived from the freeze documents.

## How to build and test it

The human runs (from the repo root):

```sh
cargo test -p flexcore-domain          # unit + contract tests for the frozen contracts
cargo clippy -p flexcore-domain -- -D warnings
cargo fmt -p flexcore-domain -- --check
python tools/verify_docs.py            # report-tree structural checks (unchanged inputs)
python tools/validate_agent.py         # validates .agent/ files (DECISIONS.md was updated)
```

Suggested first review order: `11_command_state_machine.md` and
`command_state.rs` (the Phase 4 test oracle), then
`07_units_sign_conventions.md`/`units.rs`, then `26_foundations_crosscheck.md`.

## Blockers

1. **P0.29 — Stage 0 exit-gate review NOT HELD.** It requires engineering,
   product, and power-systems reviewers; an agent cannot convene or represent
   them and must not record a pass on their behalf. Agenda and prepared
   inputs: `29_exit_gate_review.md`. ADR-001…012 stay "Proposed" until it
   passes.
2. **P0.28 — named owners and target dates for D1–D6.** The ledger forbids
   inventing persons or calendar dates; the gate-table skeleton with owner
   roles, venues, and the escalation rule is in `03_decision_records.md` §7.
   Fill it at the gate meeting, then mirror into `.agent/DECISIONS.md`.
3. **P0.01 — reviewer sign-offs** (five groups) are blank by design; humans
   sign in `01_scope_and_non_goals.md`.

## Decisions still needed

- D1 (jurisdiction/market) — product + legal; scheduled, not decided
  (`03_decision_records.md` §1).
- D2 (first portfolio and service) — product; buyer named as a RECOMMENDATION
  only; baseline-method question OPEN pending D1 (§2).
- D3 (network class/fidelity) — power systems; balanced-radial 0.x default is
  a recommendation with a recorded revisit trigger (§3).
- D4 (license/dependency audit) — legal + engineering; gate "before v0.1.0";
  core license OPEN, Apache-2.0 recommended only (§4).
- D5 (first protocol/asset mix) — product + integration; gate "before v0.4.0"
  (§5).
- D6 (cyber/OT boundary review) — security; gate "before any read-only pilot" (§6).
- Foundations rules not yet adopted by any freeze doc (recorded, not filled
  in, in `26_foundations_crosscheck.md`): §9.4 per-adapter acknowledgement
  semantics (no generic 2-second timeout), §3.1 no universal control cadence.

## Assumptions

- ASSUMPTION: The coordinator loop rules ("Phase 0 … implement them as code
  contracts (types, enums, state machine, validation, constants) plus the
  decision documents") override the ledger's documents-only stance and the
  P0.30 criterion "no code files added by Phase 0 work". Deviation recorded in
  `30_anti_scope_audit.md` §1; criterion left unticked there and in
  `CHECKLIST.md`.
- ASSUMPTION (quality mapping, `quality.rs`): the freeze fixes the mapping
  rule but not each per-state choice; implemented Substituted→Estimated,
  OutOfRange→Suspect, Unsynchronized→Suspect, CommunicationLoss→Stale,
  Unreliable→Suspect, documented in the function and changeable without API
  change.
- ASSUMPTION (time model, `time_model.rs`): ingest_time ≥ event_time enforced;
  clock-quality values Synchronized/Unsynchronized/Unknown; std-only UTC i64
  millis representation (no chrono — dependency additions are the human's call).
- ASSUMPTION (audiences, `01_scope_and_non_goals.md`): §0 enumerates four
  audiences while §1.3's table lists five roles; both reproduced with a review
  note for the gate.

## Known risks

1. **`command_state.rs` is the Phase 4 test oracle** — review its frozen-edge
   table against `11_command_state_machine.md` first; any drift there
   propagates into the §22.3 release-gate evidence.
2. **Code is unbuilt.** The crate is pure std with no dependencies, but the
   human's `cargo test`/`clippy` is the first real verification; the code
   subagent reported three bugs caught and fixed by manual review, which is
   exactly the kind of thing the build will confirm.
3. **The gate is pending.** Everything above is "implemented, awaiting human
   build/test/review" — nothing is verified, and no ADR is "Accepted" yet.
4. **Foundations carry-forwards** (§9.4, §3.1) must not be forgotten when
   Phase 4 lands command acknowledgement and scheduling cadence.

## Batch plan (written before delegation, per loop rules)

Phase 0 subtasks are implemented in dependency order in five batches. Two
implementation tracks run in parallel per batch (file ownership is disjoint).

| Batch | Subtasks | Files owned (create/edit) | Notes |
|---|---|---|---|
| 1 | P0.01, P0.02, P0.24 | `01_scope_and_non_goals.md`, `02_first_buyer_use_cases.md`, `24_vertical_slice_map.md` | Extract §0.3/§1.3/§1.6/§1.5/§24 verbatim from the report |
| 1 | P0.03, P0.04, P0.05, P0.06 | `03_decision_records.md` | D1–D6 scheduled/recorded; no decision invented; `.agent/DECISIONS.md` is updated by the coordinator only |
| 2 | P0.07, P0.08, P0.09, P0.10 | `07_units_sign_conventions.md`, `08_telemetry_quality_vocabulary.md`, `09_time_model.md`, `10_asset_capability_vocabulary.md` | Report §10, §11, §2.2 |
| 2 | P0.11, P0.12, P0.13, P0.14 | `11_command_state_machine.md`, `12_grid_model_contract.md`, `13_solver_class_discipline.md`, `14_event_vocabulary.md` | Report §14, §12, §7.4, §15.3, §6.3 |
| 3 | P0.15, P0.16, P0.20, P0.22 | `15_api_surface.md`, `16_tenancy_storage_principles.md`, `20_deployment_build_form.md`, `22_console_scope.md` | Report §15, §16, §21, §20 |
| 3 | P0.17, P0.18, P0.19, P0.23 | `17_threat_model.md`, `18_control_authority.md`, `19_plugin_model.md`, `23_simulation_test_strategy.md` | Report §17, §21.4, §5, §19 |
| 4 | P0.21 | `docs/architecture/adr/ADR-001.md` … `ADR-012.md` | Written after batches 1–3 provide the evidence; status "proposed" |
| 4 | P0.26 | `26_foundations_crosscheck.md` | Reads foundations v1.0 §6 from `docs/foundations/*.pdf` (canonical, read-only) |
| 5 | P0.07/P0.08/P0.09/P0.11/P0.14/P0.15 (code contracts) | `Cargo.toml` (workspace root), `crates/flexcore-domain/**` | Loop rule 2: frozen contracts implemented as types/enums/state machine/constants + tests. Deviation vs ledger "documents only" stance is recorded below |
| close | P0.25, P0.27, P0.28, P0.29, P0.30 | `27_handover_pack.md`, `29_exit_gate_review.md`, `30_anti_scope_audit.md`, `.agent/DECISIONS.md`, `RESEARCH.md`, `ledger_phase_0.md`, `CHECKLIST.md` | Coordinator-only |

P0.25 evidence already exists in `RESEARCH.md` §5 (checked on the cut-off
date, 7 November 2026); the close step confirms it and ticks the ledger.
