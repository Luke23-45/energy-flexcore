# P0.27 — Phase 0 handover pack for Phase 1

Provenance: subtask P0.27 (ledger_phase_0.md). Date: 2026-11-07. Status: draft for the Stage 0 exit-gate review (P0.29). Coordinates with `HANDOVER.md` (build/test instructions, blockers, assumptions, risks).

## 1. Handover artefacts (P0.01–P0.26)

| # | Artefact | Subtask | Location | Status |
|---|---|---|---|---|
| 1 | Scope and non-goals note | P0.01 | `ledger/phase-00-product-freeze/01_scope_and_non_goals.md` | Frozen (draft; reviewer sign-off lines open) |
| 2 | First buyer and first use cases | P0.02 | `ledger/phase-00-product-freeze/02_first_buyer_use_cases.md` | Frozen (buyer = recommendation pending D2) |
| 3 | D1–D6 decision / scheduled-gate records | P0.03–P0.06 | `ledger/phase-00-product-freeze/03_decision_records.md`; `.agent/DECISIONS.md` | Scheduled — no D1–D6 decision made |
| 4 | Canonical units and sign conventions | P0.07 | `ledger/phase-00-product-freeze/07_units_sign_conventions.md`; code: `crates/flexcore-domain/src/units.rs` | Frozen |
| 5 | Telemetry quality vocabulary | P0.08 | `ledger/phase-00-product-freeze/08_telemetry_quality_vocabulary.md`; code: `crates/flexcore-domain/src/quality.rs` | Frozen |
| 6 | Time model | P0.09 | `ledger/phase-00-product-freeze/09_time_model.md`; code: `crates/flexcore-domain/src/time_model.rs` | Frozen |
| 7 | Asset and capability vocabulary | P0.10 | `ledger/phase-00-product-freeze/10_asset_capability_vocabulary.md` | Frozen (vocabulary only; Phase 3 implements models) |
| 8 | Command state machine | P0.11 | `ledger/phase-00-product-freeze/11_command_state_machine.md`; code: `crates/flexcore-domain/src/command_state.rs` | Frozen (Phase 4 test oracle) |
| 9 | Grid-model contract | P0.12 | `ledger/phase-00-product-freeze/12_grid_model_contract.md` | Frozen (no provider selected; Phase 4, gated on D3) |
| 10 | Solver-class discipline | P0.13 | `ledger/phase-00-product-freeze/13_solver_class_discipline.md` | Frozen |
| 11 | Event vocabulary (26 events) | P0.14 | `ledger/phase-00-product-freeze/14_event_vocabulary.md`; code: `crates/flexcore-domain/src/events.rs` | Frozen |
| 12 | API resource surface (16 resources) | P0.15 | `ledger/phase-00-product-freeze/15_api_surface.md`; code: `crates/flexcore-domain/src/api_paths.rs` | Frozen (candidate list; Phase 1 implements) |
| 13 | Tenancy and storage principles | P0.16 | `ledger/phase-00-product-freeze/16_tenancy_storage_principles.md` | Frozen |
| 14 | Threat model and security zones | P0.17 | `ledger/phase-00-product-freeze/17_threat_model.md` | Frozen (system requirements, not a certification claim) |
| 15 | Control-authority modes | P0.18 | `ledger/phase-00-product-freeze/18_control_authority.md` | Frozen (0.x simulation-only) |
| 16 | Plugin model and protected kernel | P0.19 | `ledger/phase-00-product-freeze/19_plugin_model.md` | Frozen |
| 17 | Deployment and build form | P0.20 | `ledger/phase-00-product-freeze/20_deployment_build_form.md` | Frozen |
| 18 | ADR-001…ADR-012 | P0.21 | `docs/architecture/adr/ADR-001.md` … `ADR-012.md` | Proposed (pending P0.29; next ADR is ADR-013) |
| 19 | Console scope (twelve workspaces) | P0.22 | `ledger/phase-00-product-freeze/22_console_scope.md` | Frozen (console is Phase 5 work) |
| 20 | Simulation / testing / benchmark strategy | P0.23 | `ledger/phase-00-product-freeze/23_simulation_test_strategy.md` | Frozen (plan, not a suite; no thresholds set) |
| 21 | Vertical-slice map (§24, 10 steps) | P0.24 | `ledger/phase-00-product-freeze/24_vertical_slice_map.md` | Frozen (not all in v0.1.0) |
| 22 | Research cut-off check | P0.25 | `ledger/phase-00-product-freeze/RESEARCH.md` §5 | Complete on 2026-11-07 (the cut-off date itself) |
| 23 | Foundations cross-check | P0.26 | `ledger/phase-00-product-freeze/26_foundations_crosscheck.md` | Complete — zero unresolved conflicts |
| 24 | Phase 0 code contracts (workspace + `flexcore-domain` crate with tests) | P0.07/P0.08/P0.09/P0.11/P0.14/P0.15 (code aspect) | `Cargo.toml`, `crates/flexcore-domain/` | Written, NOT built, NOT tested (human runs `cargo test`) |

Citation rule (from `PLAN.md` §4): later phases cite these contracts by ID; they do not re-define them. A change to a frozen contract requires a new ADR and a re-run of the affected gate, not a quiet edit.

## 2. Open questions Phase 1 must not answer by itself

| Question | Owning gate / subtask |
|---|---|
| D1 target jurisdiction and market | D1 (product + legal) — scheduled, see `03_decision_records.md`; P9.11 / P10.27 carry it if still open |
| D2 first portfolio and service | D2 (product) — buyer named as recommendation only; baseline-method question (§11.4) is OPEN |
| D3 network class and model fidelity | D3 (power systems) — 0.x default is a recommendation, not a decision; Phase 4 provider selection is gated on it |
| D4 license and dependency audit | D4 (legal + engineering), gate "before v0.1.0"; P1.27/P1.28 own the inventory + cargo-deny gate |
| D5 first protocol and asset mix | D5 (product + integration), gate "before v0.4.0"; P6.01 owns it |
| D6 cyber and OT boundary review | D6 (security), gate "before any read-only pilot" |
| Named owners and target dates for all six gates | Stage 0 gate meeting (P0.28) — blanks left deliberately, ledger forbids invented dates/persons |
| §23 Stage 0 exit-gate outcome | P0.29 — requires engineering, product, and power-systems reviewers; cannot be held by an agent |
| Foundations rules not yet adopted by a freeze doc (§9.4 per-adapter ack semantics; §3.1 no universal control cadence) | Recorded in `26_foundations_crosscheck.md`; Phase 4 adopts them where they bind |

## 3. Phase 1 entry condition

The §23 Stage 0 exit gate (P0.29) has NOT passed — it requires human reviewers
(engineering, product, power systems). Per the implementation coordinator's
loop rules, Phase 1 implementation may proceed on the frozen contracts above
while the gate review is scheduled; the gate must be recorded before any
release claim (v0.1.0) is made. Physical control work remains blocked until
the relevant gates close (§23 Stage 0; nothing in Phase 0 or Phase 1
authorises field control).
