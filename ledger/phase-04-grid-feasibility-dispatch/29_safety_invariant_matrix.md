# Decision record — P4.29 Safety invariants enforcement (§17.5)

**Status:** implemented, awaiting human build/test. **Nothing was built,
installed, or executed** — no `cargo build/check/test/clippy/fmt`, no test
runner, no package install (AGENTS.md working agreement; the loop's
build/test/install bans). All test names below are pre-registered UNRUN
except where cited from already-written suites (equally UNRUN). The human
build/test pass is the gate.

## 1. Authority model this matrix rests on (cited, not from memory)

| Claim | Source (read, quoted) |
|---|---|
| The protected kernel is "authorization, tenant isolation, audit integrity, command lifecycle, final dispatch validation. No plugin may replace these." | `.agent/CONTEXT.md`, *Architecture in one screen* |
| "Mode is a deployment/site setting. Plugins cannot select or raise it." | `.agent/CONTEXT.md`, *Control authority* |
| "safe-by-construction GNN/MARL outputs" are deliberately not promised; a research output is never trusted by construction | `.agent/CONTEXT.md`, *What is deliberately not promised* |
| "Hard real-time or protection-grade control" is deliberately not promised | `.agent/CONTEXT.md`, *What is deliberately not promised* |
| "No OT boundary crossed; no pilot; P4.28–P4.29 are code-level controls only" | `ledger/phase-04-grid-feasibility-dispatch/CONTEXT.md`, D6 row |

Consequence for the matrix: the ten §17.5 invariants are proven as
**code-level controls in the protected domain/application layer**, in the
simulation-only 0.x line. No safety, certification, protection-grade, or
transport-security claim is made anywhere in this record.

## 2. The invariant-to-test matrix

Enforcement points are given as `file :: operation`, taken from the actual
code (read, not assumed). Every invariant maps to REAL existing or new
tests; all suites live under `crates/`.

### Invariant 1 — valid asset/site identity and current command context

| | |
|---|---|
| Enforcement point | `flexcore-domain/src/tenant_context.rs` :: `TenantContext::from_authenticated_claims` (the ONLY public construction path — a forged tenant cannot drive use cases); `flexcore-application/src/command_gates.rs` :: `create_proposed_command` + `run_validation_gate` (asset exists in the caller's tenant via the `AssetCapabilitySource` seam, scope IDs carried and validated by `DispatchCommand::try_new`; unknown/foreign assets reject without leaking existence) |
| Tests | `flexcore-domain/tests/security_tenant_context.rs` :: `from_verified_is_the_only_public_path_and_reachable_from_outside_the_domain`; `flexcore-application/tests/security_isolation.rs` :: `tenant_b_cannot_read_any_of_tenant_as_entities`, `a_forged_tenant_cannot_drive_the_use_cases_at_all`; `flexcore-application/tests/command_gates.rs` :: `an_unknown_asset_rejects_without_leaking_existence`; `flexcore-application/tests/rejection_invalid.rs` :: `f_iv_4_an_unknown_or_foreign_asset_is_rejected_at_the_validation_gate`; `flexcore-application/tests/dispatch_simulator.rs` :: `unknown_or_unusable_assets_are_rejected_at_the_validation_gate_before_any_issue` |
| Layer | Domain + application. No gap recorded. |

### Invariant 2 — capability validation

| | |
|---|---|
| Enforcement point | `flexcore-application/src/command_gates.rs` :: `run_validation_gate` (capability model present, quantities declared, ramp against the declared capability); `flexcore-application/src/command_record.rs` :: `outside_capability_rejection` / `ramp_outside_capability_rejection` (the §14.3 per-dimension verdict layer); `flexcore-domain` capability models (per-asset-class validation, P3.01/04) |
| Tests | `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_capability_exceedance_is_rejected_naming_the_asset`, `an_undeclared_capability_rejects_fail_closed`; `flexcore-application/tests/command_gates.rs` :: `a_missing_capability_model_and_undeclared_quantities_are_rejected`, `ramp_and_duration_policy_checks_reject_with_named_checks`; `flexcore-application/tests/command_record.rs` :: `rejection_rule_outside_capability_fires_per_target_dimension`, `rejection_rule_outside_capability_covers_the_ramp_requirements`; `flexcore-application/tests/rejection_invalid.rs` :: `f_iv_2_a_kw_scaled_value_at_the_mw_boundary_is_outside_capability_not_rescaled`, `f_iv_5_a_target_outside_the_declared_capability_is_rejected_with_the_rule_named`; `flexcore-domain/tests/capability_validation.rs` :: `negative_ramp_rejected_with_named_error` (and the per-class suite) |
| Layer | Domain + application. No gap recorded. |

### Invariant 3 — required network feasibility evidence for the declared operating profile

| | |
|---|---|
| Enforcement point | `flexcore-application/src/command_gates.rs` :: `run_feasibility_gate` (§12.5 pipeline through the `CommandFeasibilityRunner` seam; feasibility evidence is a DISTINCT reference from validation evidence and binds the model hash) and `ensure_command_issueable` (the P4.16 issue hook: the evidence's model hash must equal the ACTIVE model version's hash, else refuse — §17.5 invariant 3 verbatim in the module header); `flexcore-application/src/dispatch_simulator.rs` :: `issue` step 4 (calls the hook before ISSUED) |
| Tests | `flexcore-application/tests/command_gates.rs` :: `the_feasibility_gate_moves_validated_to_feasible_with_separate_evidence`, `feasibility_evidence_binds_the_model_hash_and_a_model_change_blocks_issue`, `the_issue_hook_requires_approved_and_fails_closed_on_missing_evidence`, `a_fail_closed_verdict_rejects_never_passes`; `flexcore-grid-api/tests/feasibility_gate.rs` :: `the_pipeline_completes_in_order_with_every_intermediate_evidence_row` |
| Layer | Application (gate) + application/adapter (issue hook). No gap recorded. |

### Invariant 4 — authorization (P4.23 is the recorded suite)

| | |
|---|---|
| Enforcement point | `flexcore-application/src/command_gates.rs` :: `record_command_approval` (the ONLY operation that can produce APPROVED: deny-by-default evaluator decision, scope coverage of the command's asset, separation of duties, non-expired authorization; the decision record carries actor, role, scope, limits, expiry, and the EXACT validation + feasibility evidence IDs covered); `flexcore-authz/src/phase4_policies.rs` + `policy.rs` (deny-by-default policy surface; `dispatch.authorize`/`dispatch.issue` are OFF the action surface); `flexcore-domain/src/command_lifecycle.rs` :: `record_authorization` (FEASIBLE-only, structural) |
| Tests | `flexcore-application/tests/rejection_unauthorized.rs` (P4.23, 9 tests) :: `an_unauthorized_role_is_rejected_with_the_pre_registered_error`, `the_correct_role_at_tenant_scope_is_denied_no_tenant_level_approval_grant`, `approval_by_the_proposer_is_rejected_where_separation_of_duties_is_configured`, `an_approval_with_an_expired_authorization_scope_is_rejected`, `every_pre_registered_outcome_holds_and_no_fixture_reaches_issued`; `flexcore-application/tests/command_gates.rs` :: `approval_is_impossible_without_feasible_the_guard_at_both_layers`, `approval_by_a_distinct_authorized_actor_records_the_full_decision`; `flexcore-authz/tests/phase4_policies.rs` :: `approval_is_granted_to_tenant_admin_at_site_and_asset_scope_only`, `the_empty_set_denies_the_whole_command_surface` |
| Layer | Domain (state structure) + application (decision) + authz crate (policy). No gap recorded. |

### Invariant 5 — no stale/expired command

| | |
|---|---|
| Enforcement point | `flexcore-domain/src/command_lifecycle.rs` :: `record_expiry` + `validity_has_lapsed` (EXPIRED never fires early; milestone must match the state; controlled clock only); `flexcore-application/src/command_expiry.rs` :: `CommandExpiryService` (the sweep); `flexcore-application/src/command_record.rs` :: `stale_rejection` (the §14.3 stale rule); `flexcore-application/src/dispatch_simulator.rs` :: `check_issue_refusals` (expired-at-issue refusal) + `issue` (validity checks from the loaded record) |
| Tests | `flexcore-application/tests/rejection_expired.rs` (P4.25) :: `expiry_never_fires_before_the_window_ends_and_the_boundary_is_end_exclusive` + the four milestone fixtures; `flexcore-application/tests/rejection_stale.rs` :: `f_st_1_a_command_whose_window_ended_before_validation_is_rejected_stale_not_expired`, `f_st_5_a_command_already_approved_when_validity_ended_expires_with_the_milestone`; `flexcore-application/tests/command_expiry.rs` :: `the_sweep_decides_exactly_the_expirable_states_with_lapsed_windows`; `flexcore-application/tests/dispatch_simulator.rs` :: `expired_approval_cannot_be_issued_and_expires_via_the_sweep`, `issue_refusals_fire_unsigned_unsequenced_and_expired_in_ledger_order` |
| Layer | Domain (state + comparison) + application (sweep, issue refusal). No gap recorded. |

### Invariant 6 — no unapproved adapter/target (P4.16 target guard; re-verified here)

| | |
|---|---|
| Enforcement point | `flexcore-application/src/dispatch_simulator.rs` :: `SimulatorDispatchAdapter::configure` — the D-P4.16-6 HARD GUARD: only `AdapterTargetCandidate::Simulator` is accepted; every `NonSimulator` candidate (gateway, protocol adapter, device, field endpoint) is refused with a named error; `SIMULATION_ONLY_STATEMENT` records the boundary in the public API |
| Tests | Existing: `flexcore-application/tests/dispatch_simulator.rs` :: `the_target_guard_refuses_every_non_simulator_kind`, `the_transport_port_has_exactly_deliver_and_observe_and_no_write_path`. NEW re-verification (this subtask): `flexcore-application/tests/invariant7_plugin_bypass.rs` :: `the_only_dispatch_target_is_the_simulator_every_non_simulator_target_is_refused` (all four `NonSimulatorTargetKind` values refused with kind + endpoint recorded; the simulator candidate accepted; `SIMULATION_ONLY_STATEMENT` pinned) |
| Layer | Application (adapter constructor — a composition root cannot configure a field target without the constructor refusing it). No gap recorded. |

### Invariant 7 — no direct physical command from optimizer/GNN/MARL/LLM/plugin

| | |
|---|---|
| Enforcement point | STRUCTURAL, at three protected layers. (a) Domain: `flexcore-domain/src/command_lifecycle.rs` — `CommandLifecycle::propose` is the sole constructor, the state starts at PROPOSED, fields are private, and every transition goes through the frozen-edge oracle `command_state::try_transition` (no PROPOSED → ISSUED edge exists; gates apply only from their own states). `flexcore-domain/src/dispatch_command.rs` :: `try_new` is the only command-record construction and validates every §14.3 group. (b) Application: `flexcore-application/src/command_gates.rs` — commands are created FROM AN APPROVED SCHEDULE ONLY and pass through the three distinct gates; `SourceOrigin` (subsystem + plugin version) is RECORDED (§14.3 field 3) and confers no authority — no gate branch, flag, or parameter exists that short-circuits the chain for any source. (c) Adapter: `flexcore-application/src/dispatch_simulator.rs` :: `issue` — APPROVED-only, refuses unsigned/unsequenced/expired, cross-checks the authorization record against the journal's APPROVED evidence, re-validates the §14.3 record, re-checks feasibility evidence against the ACTIVE model hash, and delivers ONLY to the injected simulator transport |
| Tests | NEW (this subtask): `flexcore-application/tests/invariant7_plugin_bypass.rs` — header `// P4.29 — Invariant 7: no research-module direct dispatch`; three tests: `a_research_plugin_command_enters_only_at_proposed_and_no_direct_issue_edge_exists` (calls the DOMAIN directly, per the ledger risk mitigation: a research-plugin-shaped actor cannot record an issue, feasibility, or authorization transition out of order — `NotPermitted` / `WrongState`, journal stays empty); `a_research_plugin_proposal_reaches_issued_only_through_the_full_gate_chain` (a proposal whose `SourceOrigin` is `research:gnn-marl-experimental / plugin/0.0.0-unqualified` walks create → validate → feasible → approve → issue through the REAL gates; the authorization decision record must cover the EXACT validation + feasibility evidence IDs; application-layer skips on a PROPOSED command are refused; the only delivery is to the simulator transport with the `sim://dispatch-adapter` identity); `the_research_plugin_source_confers_no_authority_deny_by_default_holds` (with no matching grant the research-shaped proposal is REJECTED at the authorization gate and issue is refused). Existing structural oracles: `flexcore-domain/tests/transition_matrix.rs` :: `no_edge_outside_the_frozen_table_exists`, `the_ledgers_named_gate_skipping_shortcuts_are_illegal_cells` (P4.33 matrix); `flexcore-domain/tests/command_lifecycle.rs` :: `approved_without_feasible_evidence_is_impossible_by_construction` |
| Layer | Domain + application + adapter. **Structural note:** a plugin cannot bypass what it cannot construct — there is no public constructor for a non-PROPOSED lifecycle and no source-based shortcut in the gate chain. No gap recorded. |

### Invariant 8 — no cross-tenant access

| | |
|---|---|
| Enforcement point | `flexcore-domain/src/tenant_context.rs` (verified-claims-only context) + the tenant-scoped `CommandGateStore` seam semantics (P0.16 rule 5: a foreign command loads as `None` — existence never leaks) implemented by the persistence layer (`crates/flexcore-storage`, `crates/adapters/postgres/src/command_store.rs`); `flexcore-authz/src/policy.rs` :: cross-tenant scope denial; `flexcore-application/src/dispatch_security.rs` :: `record_tenant_denial` + `SecurityAuditSink` (the audited-denial half, P4.28) |
| Tests | `flexcore-application/tests/dispatch_simulator.rs` :: `a_cross_tenant_caller_cannot_issue_a_foreign_command`; `flexcore-application/tests/rejection_unauthorized.rs` :: `a_cross_tenant_approval_attempt_is_blocked_existence_never_leaks`; `flexcore-authz/tests/security_authz.rs` :: `cross_tenant_scope_denies_even_for_platform_admin`, `cross_tenant_denial_holds_for_every_role_with_a_grant`; `flexcore-application/tests/dispatch_security.rs` (P4.28) :: `a_cross_tenant_read_attempt_is_denied_existence_never_leaks_and_the_denial_is_audited`, `a_cross_tenant_approval_attempt_is_denied_and_audited`, `a_cross_tenant_issue_attempt_is_denied_and_audited`; `flexcore-application/tests/security_isolation.rs` :: `tenant_b_cannot_read_any_of_tenant_as_entities` |
| Layer | Domain (context) + store seam (query scoping) + authz (policy). No gap recorded. |

### Invariant 9 — missing, uncertain, or stale model/telemetry is NOT a pass

| | |
|---|---|
| Enforcement point | `flexcore-grid-api/src/fail_closed.rs` (the D-P4.08-1 policy table: every trigger routes to REJECTED or HELD — the action vocabulary has NO pass); `flexcore-grid-api/src/model_health.rs` (content hash, staleness bound, coverage findings; only an ACTIVE model is usable); `flexcore-grid-api/src/feasibility.rs` (a stale model aborts BEFORE the power flow; a mid-gate model change aborts at the hash hook); `flexcore-application/src/command_gates.rs` :: `run_validation_gate` (P3.13 envelope freshness, grid model ACTIVE per P4.03; `a_seam_failure_is_fail_closed_never_a_pass`) and `run_feasibility_gate` (a `FailClosed` verdict REJECTS, never passes) |
| Tests | `flexcore-grid-api/tests/fail_closed.rs` :: `every_trigger_produces_a_fail_closed_outcome_naming_the_trigger`, `the_fail_closed_action_vocabulary_has_no_pass`, `every_pipeline_error_classifies_to_a_fail_closed_outcome`; `flexcore-grid-api/tests/model_health.rs` :: `only_an_active_model_is_usable`, `a_source_change_after_verification_makes_the_model_stale`, `an_uncovered_element_is_a_finding_and_blocks_usability`; `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_stale_model_aborts_before_the_power_flow`, `a_mid_gate_model_change_aborts_at_the_hash_hook`, `bad_missing_and_stale_baselines_block_the_ramp_check`, `a_failing_seam_rejects_fail_closed`; `flexcore-application/tests/command_gates.rs` :: `an_inactive_or_unknown_model_rejects_at_the_validation_gate`, `a_seam_failure_is_fail_closed_never_a_pass`, `a_fail_closed_verdict_rejects_never_passes`; `flexcore-application/tests/rejection_stale.rs` :: `f_st_3_a_stale_telemetry_quality_event_makes_the_envelope_stale_and_the_gate_rejects` |
| Layer | Grid crate (pipeline + health) + application (gates). No gap recorded. |

### Invariant 10 — site-local safety/protection independent of FlexCore (BOUNDARY NOTE — no code test)

| | |
|---|---|
| Boundary note | The §17.5 invariant that site-local safety/protection functions are independent of FlexCore is a DOCUMENTED BOUNDARY, not a code test — per the ledger step ("documented as a boundary, not a code test"). **How the simulator models local protection: it does not.** Verified by reading `crates/flexcore-simulator/src` (lib.rs, scenario.rs, source.rs, engine.rs, profiles.rs, rng.rs): the Phase 2 simulator models synthetic telemetry behaviour profiles (BESS/PV/flexible-load/EVSE) and the seven telemetry fault kinds for testing the platform's handling of missing/stale/duplicate values — it contains NO modelling of site-local protection devices, relays, or their independence from FlexCore. This is a recorded boundary, never a claim. The crate's own scope statement pins the direction: "FlexCore is a supervisory orchestration layer and performs no primary frequency response, synthetic inertia, relay protection, inverter inner-loop control, or sub-cycle control" (`crates/flexcore-simulator/src/lib.rs`, *Scope and control status*), matching `.agent/CONTEXT.md` *What is deliberately not promised* ("Hard real-time or protection-grade control") and the report §5.7/§14.5 control status carried in every Phase 4 module header. Independence of site-local protection is therefore an INPUT to FlexCore's design (FlexCore commands are supervisory and never a substitute for local protection), not a property FlexCore implements or tests. Any future simulator modelling of local protection is new scope with its own decision record. |

## 3. Enforcement-layer check (the ledger risk: "enforced only in the API layer")

The ledger risk asks whether an invariant could be bypassed by an internal
caller. Findings, honestly recorded (no architecture was restructured):

| Invariant | Where the enforcement lives | Gap? |
|---|---|---|
| 1, 8 | Domain (`tenant_context`) + store-seam semantics + authz policy; every gate/adapter operation takes `&TenantContext` and scopes through it | No. There is no REST/route layer yet (P4.30 wires it); there is no outer layer to bypass. |
| 2 | Domain capability models + application validation gate + §14.3 verdict layer | No. |
| 3 | Application gate + the P4.16 adapter calls `ensure_command_issueable` before ISSUED | No. An internal caller could call `CommandLifecycle::record_issue` directly — but that is the frozen-edge oracle's edge check, tested at the domain layer (invariant 7). |
| 4 | Application `record_command_approval` (sole APPROVED producer) + authz policy set | No. |
| 5 | Domain `record_expiry` (never fires early) + application sweep + adapter refusal | No. |
| 6 | Application adapter constructor (the guard is in `configure`, not a route) | No. |
| 7 | Structural: domain propose-only entry + frozen edges; application gate chain; adapter APPROVED-only + simulator-only delivery | No — see the structural note in the invariant 7 row. |
| 9 | Grid crate fail-closed policy + model health + application gates | No. |
| 10 | Not applicable (boundary, not code) | Not applicable. |

**Recorded residual gaps (none silently filled):**

1. **Uniqueness and monotonic sequence are caller-side duties** by design
   (`flexcore-domain/src/dispatch_command.rs` header): the constructor
   cannot observe them; they are enforced at the persistence/registry layer
   (store `Conflict` + `TransitionConflict`, the P4.24 dedup, the
   `SequenceGuard`) and tested there — the domain constructor alone is not
   the enforcement point for these two.
2. **The P4.16 issue path still records the legacy traceability signature**;
   payload-bound verification is enforced at the receive point
   (`VerifyingDispatchTransport`, P4.28) but wiring the adapter's issued
   envelopes to it is composition-root/Stage-6 work (recorded in
   `28_security_controls.md`, not silently assumed here).
3. **The plugin kernel does not exist yet** (v0.5.0 per the release
   ladder). "Cannot be bypassed by plugins" is therefore proven
   structurally: no plugin execution path exists in this slice, the gate
   chain exposes no bypass parameter or source-based shortcut, and the
   tests prove the chain from both the domain and the application layer.
   The WASM/gRPC host is new scope with its own authority model.
4. **The seams are injected** (`CommandGateStore`, `AssetCapabilitySource`,
   …): a composition root could in principle inject a permissive store.
   Mitigated, not eliminated: the tests call the domain engine directly
   (the frozen-edge oracle is not injectable), the postgres adapter
   implements the tenant-scoped semantics (P4.10), and the dependency-rule
   check (`tools/check_dependency_rule.py`) pins who may depend on what.
   Recorded as an honest limitation of seam-based architectures.

## 4. The new test file (pre-registered UNRUN)

`crates/flexcore-application/tests/invariant7_plugin_bypass.rs` — 3 tests,
test-only over existing public APIs (no src edit, no new dependency edge):

| Test | Proves |
|---|---|
| `a_research_plugin_command_enters_only_at_proposed_and_no_direct_issue_edge_exists` | Domain layer, called directly: PROPOSED → ISSUED is refused (`LifecycleError::NotPermitted`); `record_feasibility` / `record_authorization` out of order are refused (`WrongState`); the journal stays empty. The lifecycle enters only at PROPOSED (`propose` is the sole constructor). |
| `a_research_plugin_proposal_reaches_issued_only_through_the_full_gate_chain` | Application layer, real gates: the research-shaped proposal skips are refused; the full chain issues; the approval record covers the exact validation + feasibility evidence; all three gates appear in the journal in order; the only delivery is to the simulator transport with the `sim://dispatch-adapter` identity; the emitted event set is the frozen one. |
| `the_research_plugin_source_confers_no_authority_deny_by_default_holds` | With no grant, the research-shaped proposal is REJECTED at the authorization gate (deny-by-default, §17.3) and the adapter refuses issue from REJECTED; nothing is delivered. |
| `the_only_dispatch_target_is_the_simulator_every_non_simulator_target_is_refused` | Invariant 6 re-verification: all four `NonSimulatorTargetKind` values refused with kind + endpoint; the simulator candidate accepted; `SIMULATION_ONLY_STATEMENT` pinned; no delivery for any refused target. |

## 5. Review status

The ledger acceptance criteria name review by security + power-systems.
That review has NOT happened (loop roles are not human reviewers). Recorded
as the gate input, matching the P4.27 sign-off pattern: **NOT SIGNED**.

## 6. File/test index (this subtask's footprint)

| File | Status |
|---|---|
| `ledger/phase-04-grid-feasibility-dispatch/29_safety_invariant_matrix.md` | NEW (this file) |
| `crates/flexcore-application/tests/invariant7_plugin_bypass.rs` | NEW, 4 tests, pre-registered UNRUN |
| `ledger/phase-04-grid-feasibility-dispatch/ledger_phase_4.md` | P4.29 checkbox ticked (implemented, awaiting human build/test) |
| `ledger/phase-04-grid-feasibility-dispatch/HANDOVER.md` | Wave 6a row updated |

No `src/**` file was touched. Nothing was built, installed, or run.
