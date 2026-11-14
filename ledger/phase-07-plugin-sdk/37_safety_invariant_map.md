# P7.37 — §17.5 safety-invariant test map for the plugin SDK

**Status:** implemented, awaiting human build/test. Nothing was built,
installed, or executed — the loop bans hold (AGENTS.md §3 rule 7; the
ENTRY_NOTE discipline). Every test named below is WRITTEN, UNRUN until the
human build/test pass executes it; names of already-written suites are
cited from the suites themselves and from
`ledger/phase-04-grid-feasibility-dispatch/29_safety_invariant_matrix.md`
(the Phase 4 map this one plugs into). Nothing here claims a test passed.

**Report trace:** §17.5 (invariants 1–10, quoted verbatim per row); §14.3;
§14/§21.4 (the control-authority boundary); §13.4; ADR-005/ADR-010.
**Companion decision record:**
`37_38_39_invariants_revocation_tenancy.md`.

---

## 1. How to read this map

The §17.5 rules are **system requirements enforced by protected platform
code** (report §17.5: "These are system requirements, not a certification
claim"). They are NOT all enforceable at the plugin SDK layer: several live
structurally in the domain/application protected path (Phase 3/4) and
cannot — and must not — be re-implemented in the kernel crate (the kernel
is a composition crate; it "must not become a second domain layer",
`crates/flexcore-kernel/src/lib.rs` header).

Each row therefore records THREE things honestly:

1. **The rule, verbatim** from report §17.5 (never paraphrased into a new
   meaning).
2. **Where the rule is enforced** — the SDK-level enforcement point(s)
   AND, where the SDK cannot enforce it, the Phase 3/4 enforcement point
   that actually upholds it (cited, not claimed as SDK coverage).
3. **The proving test(s)** — exact test file + test fn for every cited
   test. Where a row has no test at the SDK layer, it is marked
   **MAP-GAP** with the owning wave — never covered by prose.

The SDK-enforceable rows are additionally exercised by the regression
matrix `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` — the
test-side twin of this map; if an enforcement point regresses, the matrix
test fails.

**Honesty frame:** all suites cited are code-level controls in the
simulation-only 0.x line. No safety, certification, protection-grade, or
transport-security claim is made anywhere in this map (AGENTS.md §3 rules
2/5; the P4.29 authority model).

---

## 2. The invariant rows

### Invariant 1 — "No command without a valid asset/site identity and current command context."

| | |
|---|---|
| SDK enforcement points | The plugin path contributes the IDENTITY half only: a plugin request/response carries payload-bound identity (correlation id + digest + sequence + validity window — `crates/flexcore-kernel/src/plugin_path_protection.rs`), and a proposal carries its producer plugin id+version (`PluginProposalEnvelope`, the §9.2 provenance discipline). The ASSET/SITE identity and the CURRENT COMMAND CONTEXT are domain/application: a plugin result has no asset identity of its own — it enters the protected command path, where the asset checks live. |
| Protected-path enforcement (cited, not SDK) | `flexcore-domain/src/tenant_context.rs` :: `TenantContext::from_authenticated_claims`; `flexcore-application/src/command_gates.rs` :: `create_proposed_command` + `run_validation_gate` (asset exists in the caller's tenant; unknown/foreign assets reject without leaking existence) — per the P4.29 matrix, invariant 1 row. |
| Tests (SDK slice) | `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` :: `the_envelope_digest_is_deterministic_and_binds_every_field`, `manifest_with_provider` (fixture), `a_protected_contract_claim_is_refused_at_the_path_and_at_the_resolver`; `crates/flexcore-kernel/tests/conformance_p7_21.rs` :: `proposal_envelope_carries_producer_identity_and_sequence`, `payload_bound_digest_is_deterministic`. |
| Tests (protected path, cited) | `flexcore-application/tests/command_gates.rs` :: `an_unknown_asset_rejects_without_leaking_existence`; `flexcore-application/tests/rejection_invalid.rs` :: `f_iv_4_an_unknown_or_foreign_asset_is_rejected_at_the_validation_gate`; `flexcore-application/tests/security_isolation.rs` :: `a_forged_tenant_cannot_drive_the_use_cases_at_all` (all pre-registered UNRUN, P4.29). |
| Residual risk | The SDK cannot check asset identity — a plugin that fabricates asset ids in its payload payload is caught only at the validation gate. Recorded honestly: the SDK slice is identity-BINDING (who produced what, when, for which request), not identity-VERIFICATION of asset references. No gap silently filled. |

### Invariant 2 — "No command without capability validation."

| | |
|---|---|
| SDK enforcement points | The never-granted permission vocabulary is refused at EVERY SDK gate: manifest (P7.05), profile ceiling (P7.15), broker grant/check re-refusal (host P7.11), enforcement door (host P7.12) — a plugin cannot hold any capability that would let it claim command authority; the capability-validation itself is the application validation gate. |
| Protected-path enforcement (cited) | `flexcore-application/src/command_gates.rs` :: `run_validation_gate` (capability model, ramp vs declared capability); `flexcore-application/src/command_record.rs` per-dimension verdicts — P4.29 invariant 2 row. |
| Tests (SDK slice) | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `never_granted_permissions_are_refused_at_every_sdk_gate`; `crates/flexcore-kernel/tests/manifest_p7_05.rs` :: `never_granted_permission_is_rejected_at_the_manifest_gate`; `crates/flexcore-kernel/tests/profile_p7_15.rs` :: `the_ceiling_vocabulary_is_closed_and_never_granted_ids_are_refused`; `apps/plugin-host/tests/broker_p7_11.rs` :: `the_never_granted_set_is_refused_even_without_any_grant_mechanics`; `apps/plugin-host/tests/enforcement_p7_12.rs` :: `a_never_granted_permission_is_refused_before_instantiation`; `apps/plugin-host/tests/sandbox_p7_13.rs` :: `no_permission_id_conveys_dispatch_authority_unbuildable_not_discouraged`. |
| Tests (protected path, cited) | `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_capability_exceedance_is_rejected_naming_the_asset`; `flexcore-application/tests/command_gates.rs` :: `a_missing_capability_model_and_undeclared_quantities_are_rejected` (P4.29). |
| Residual risk | The SDK bounds WHAT a plugin may request, not whether a specific command is within a specific asset's capability — that comparison needs the domain capability models and stays in the application layer. |

### Invariant 3 — "No command without required network feasibility evidence for the declared operating profile."

| | |
|---|---|
| SDK enforcement points | NONE. The SDK has no feasibility surface at all — by construction. The plugin-path outcome enum has no feasibility-passing variant (`PluginPathOutcome`, ADR-010); a plugin result enters the protected path as a proposal and cannot confer feasibility evidence. The evidence chain (validation evidence distinct from feasibility evidence, model-hash binding, the issue hook) is Phase 3/4. |
| Protected-path enforcement (cited) | `flexcore-application/src/command_gates.rs` :: `run_feasibility_gate` + `ensure_command_issueable` (the evidence's model hash must equal the ACTIVE model hash before ISSUED); `flexcore-grid-api/src/fail_closed.rs` + `feasibility.rs` — P4.29 invariant 3 row. |
| Tests (SDK slice) | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `the_proposal_path_has_no_dispatch_or_feasibility_authority_variant` (the exhaustive-match proof that no feasibility-authorizing variant exists); `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` :: `the_path_outcome_has_no_dispatch_authorizing_variant`. |
| Tests (protected path, cited) | `flexcore-grid-api/tests/feasibility_gate.rs` :: `the_pipeline_completes_in_order_with_every_intermediate_evidence_row`, `a_stale_model_aborts_before_the_power_flow`; `flexcore-application/tests/command_gates.rs` :: `the_feasibility_gate_moves_validated_to_feasible_with_separate_evidence`, `feasibility_evidence_binds_the_model_hash_and_a_model_change_blocks_issue` (P4.29). |
| Residual risk | None recorded at the SDK boundary: the SDK's contribution is structural (no pass variant to misuse). The feasibility gate itself is out of SDK scope by design. |

### Invariant 4 — "No command without authorization."

| | |
|---|---|
| SDK enforcement points | Structural only: the never-granted set (`dispatch.authorize`, `dispatch.issue`) is refused at every SDK gate (see invariant 2), and the plugin path has no authorization surface (the broker's validation forward returns `()` — no verdict object exists to flow back, host P7.11). The authorization DECISION is Phase 3/4. |
| Protected-path enforcement (cited) | `flexcore-application/src/command_gates.rs` :: `record_command_approval` (the only APPROVED producer); `flexcore-authz` deny-by-default policies — P4.29 invariant 4 row. |
| Tests (SDK slice) | `apps/plugin-host/tests/sandbox_p7_13.rs` :: `a_validation_verdict_authority_does_not_exist_to_request`, `grant_forgery_is_structurally_refused_and_behaviourally_futile`; `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` :: `a_proposed_dispatch_gains_no_authority_adr010`, `the_vocabulary_has_no_kernel_contract_and_no_verdict_id`; `apps/plugin-host/tests/enforcement_p7_12.rs` :: `a_proposed_schedule_gains_no_authority_from_being_proposed`, `the_validation_forward_returns_no_verdict_type_level`. |
| Tests (protected path, cited) | `flexcore-application/tests/rejection_unauthorized.rs` (P4.23, 9 tests) — e.g. `an_unauthorized_role_is_rejected_with_the_pre_registered_error`, `approval_by_the_proposer_is_rejected_where_separation_of_duties_is_configured`. |
| Residual risk | A plugin CAN request a validation forward; it can never receive a verdict through the broker (type-level). The residual is social/engineering: nothing stops a plugin author from MISTAKING a forwarded request for authorization — the tests pin that no verdict object exists to receive. |

### Invariant 5 — "No stale or expired command."

| | |
|---|---|
| SDK enforcement points | The plugin path's validity window: an expired request is refused, never executed, against the INJECTED clock only (`crates/flexcore-kernel/src/plugin_path_protection.rs` checks 3a/3b; `RequestNotYetValid`/`RequestExpired`), and the refusal never advances the sequence window (check 4 runs LAST — the P6.15 ordering). Host-side, grant expiry is checked per call (`GrantExpired`, no cached authority). |
| Protected-path enforcement (cited) | `flexcore-domain/src/command_lifecycle.rs` :: `record_expiry`; `flexcore-application/src/command_expiry.rs` sweep; `command_record.rs` stale rejection — P4.29 invariant 5 row. |
| Tests (SDK slice) | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `expiry_is_fail_closed_and_the_sequence_check_runs_last` (includes the order proof: expired-AND-replay refused as EXPIRED, window unmoved); `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` :: `an_expired_request_is_refused_and_never_executed`, `a_not_yet_valid_request_is_refused`, `a_refused_request_never_advances_the_window_in_any_refusal_class`; `apps/plugin-host/tests/broker_p7_11.rs` :: `an_expired_grant_is_refused_and_a_live_one_is_honoured`; `crates/flexcore-kernel/tests/revocation_p7_38.rs` (P7.38) — revoked keys never re-verify (the staleness analogue at the supply-chain layer). |
| Tests (protected path, cited) | `flexcore-application/tests/rejection_expired.rs` :: `expiry_never_fires_before_the_window_ends_and_the_boundary_is_end_exclusive`; `flexcore-application/tests/rejection_stale.rs` :: `f_st_1_a_command_whose_window_ended_before_validation_is_rejected_stale_not_expired` (P4.29). |
| Residual risk | Clock correctness is the injected implementation's contract; a deployment that injects a wrong clock gets consistently wrong decisions (P7.42 records the determinism posture). The SDK cannot detect a lying clock. |

### Invariant 6 — "No unapproved protocol adapter or target."

| | |
|---|---|
| SDK enforcement points | No plugin can BE an unapproved adapter or reach an unapproved target: `network.any` is never granted; there is no network capability in the §5.7 vocabulary; the broker's send-only scope admits exactly one target for `dispatch.validate.request` (`PROTECTED_VALIDATOR_TARGET`) and refuses any other at grant time; Class C worker targets are pinned by the manifest's recorded entry-point target and the kernel validator refuses a `dylib` runtime class. |
| Protected-path enforcement (cited) | `flexcore-application/src/dispatch_simulator.rs` :: `configure` — the D-P4.16-6 hard guard: only the simulator target is accepted; every non-simulator candidate is refused (P4.29 invariant 6 row). |
| Tests (SDK slice) | `apps/plugin-host/tests/broker_p7_11.rs` :: `the_validation_forward_grants_only_the_protected_validator_target`, `a_validation_forward_without_the_grant_is_denied_and_audited`; `apps/plugin-host/tests/sandbox_p7_13.rs` :: `network_sockets_are_unreachable_through_any_path`; `apps/plugin-host/tests/no_dylib_p7_35.rs` :: `the_plugin_host_crate_contains_no_dynamic_library_loading_surface`, `the_kernel_manifest_validator_still_rejects_the_dylib_runtime_class`; `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `never_granted_permissions_are_refused_at_every_sdk_gate` (includes `network.any`). |
| Tests (protected path, cited) | `flexcore-application/tests/dispatch_simulator.rs` :: `the_target_guard_refuses_every_non_simulator_kind`; `flexcore-application/tests/invariant7_plugin_bypass.rs` :: `the_only_dispatch_target_is_the_simulator_every_non_simulator_target_is_refused` (P4.29 re-verification). |
| Residual risk | The adapter APPROVAL workflow (which adapters a SITE may run) is an operator/deployment decision outside the SDK; the SDK guarantees a plugin cannot reach any target not granted, not that every granted target is approved. |

### Invariant 7 — "No direct physical command from an optimizer, GNN, MARL, LLM, or general plugin."

| | |
|---|---|
| SDK enforcement points | THE ADR-010 STRUCTURAL PROOF, at three SDK layers: (a) the kernel resolver refuses any manifest claiming a protected-kernel contract id (`PROTECTED_KERNEL_REPLACE`, ADR-005); (b) the plugin path restates the guard and its outcome enum has NO dispatch-authorizing variant (a proposal is data, never authority); (c) the host enforcement door + broker re-refuse the never-granted set at grant AND check time. There is no SDK path from a plugin output to a physical command. |
| Protected-path enforcement (cited) | Domain: `flexcore-domain/src/command_lifecycle.rs` frozen-edge oracle (no PROPOSED → ISSUED edge); application: the three distinct gates, source origin recorded but conferring no authority; adapter: APPROVED-only, simulator-only delivery — P4.29 invariant 7 row (the structural note). |
| Tests (SDK slice) | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `protected_kernel_contract_claims_are_refused_at_resolver_and_path`, `the_proposal_path_has_no_dispatch_or_feasibility_authority_variant`; `crates/flexcore-kernel/tests/resolve_p7_06.rs` :: `protected_kernel_claim_is_refused_for_every_reserved_id`, `protected_kernel_guard_fires_before_missing_provider_errors`; `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` :: `a_protected_contract_claim_is_refused_at_the_path_and_at_the_resolver`; `crates/flexcore-kernel/tests/recomposition_p7_16.rs` :: `a_protected_contract_in_the_target_is_refused_at_the_composition_level`; `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` :: `layered_never_granted_walk`, `layered_walk_dispatch_authorize_is_refused_at_every_layer`. |
| Tests (protected path, cited) | `flexcore-application/tests/invariant7_plugin_bypass.rs` :: `a_research_plugin_command_enters_only_at_proposed_and_no_direct_issue_edge_exists`, `a_research_plugin_proposal_reaches_issued_only_through_the_full_gate_chain`, `the_research_plugin_source_confers_no_authority_deny_by_default_holds`; `flexcore-domain/tests/transition_matrix.rs` :: `no_edge_outside_the_frozen_table_exists` (P4.29/P4.33). |
| Residual risk | The proof is structural (nothing to bypass), not behavioural: a future type added anywhere with dispatch authority would need its own decision record and its own refusal tests — the exhaustive-match tests here fail to compile if `PluginPathOutcome` grows a variant, which is the tripwire. |

### Invariant 8 — "No cross-tenant command or telemetry access."

| | |
|---|---|
| SDK enforcement points | P7.39 (this wave): the HOST-STAMPED `TenantScope` (kernel `crates/flexcore-kernel/src/tenant_scope.rs` :: `TenantScope::from_authenticated` — the ONLY construction path, from the platform-authenticated context, never plugin-supplied fields), bound into the request identity by `ScopedRequestEnvelope` (scope-swap/mutation = `TENANT_SCOPE_BINDING_MISMATCH`), plugin-echoed scope mismatch = `TENANT_SCOPE_ECHO_MISMATCH`, and the data-level cross-check at enforcement (`enforce_scope` / audited `enforce_scoped_request`; host mirror `apps/plugin-host/src/tenant_gate.rs` :: `TenantScopeGate::check_data_request` — envelope FIRST, then the broker's `DataScope` grant narrowing; mismatch = `TENANT_SCOPE_MISMATCH`, refused and audited). |
| RLS relationship (recorded, not re-implemented) | Plugins never touch tables (ADR-007: no authoritative state). Data access flows through the platform's tenant-scoped services; application authorization + PostgreSQL RLS remain the platform's defence in depth (report §16.2, [R25] — table owners bypass policies unless configured otherwise). The SDK adds the envelope-level guarantee only. |
| Tests (SDK slice) | `crates/flexcore-kernel/tests/tenant_scope_p7_39.rs` :: `the_scope_is_derived_from_the_authenticated_context_only`, `a_plugin_supplied_scope_that_differs_from_the_stamped_scope_is_refused`, `enforcement_refuses_any_request_outside_the_host_stamped_envelope`, `cross_tenant_attempts_are_refused_and_audited_end_to_end`, `the_scope_binding_is_deterministic_and_mutation_sensitive`; `apps/plugin-host/tests/tenant_scope_p7_39.rs` :: `the_gate_cross_checks_the_envelope_and_a_misconfigured_grant_cannot_widen_it`, `cross_tenant_attempts_are_refused_and_audited_at_both_layers`, `a_broken_audit_sink_refuses_the_gate_in_both_directions`; `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `the_tenant_scope_is_enforced_at_the_sdk_envelope`. |
| Tests (protected path, cited) | `flexcore-application/tests/dispatch_security.rs` :: `a_cross_tenant_read_attempt_is_denied_existence_never_leaks_and_the_denial_is_audited`; `flexcore-authz/tests/security_authz.rs` :: `cross_tenant_scope_denies_even_for_platform_admin`; `apps/control-plane/tests/api_edge_isolation_p6_21.rs` (the server-side scoping precedent, P6.21). |
| Residual risk | The envelope guarantee covers the plugin DATA path; the RLS layer is the platform's and its correctness (FORCE ROW LEVEL SECURITY, non-owner service roles, §16.2) is deployment configuration the SDK cannot verify. A deployment that misconfigures RLS has only the envelope + application layers — recorded, not hidden. |

### Invariant 9 — "A missing/uncertain/stale model is not interpreted as a passing feasibility check."

| | |
|---|---|
| SDK enforcement points | The SDK-side slice (the row the P7.37 review found test-less and this subtask closes): a plugin result NEVER passes feasibility through the SDK — the proposal path's outcome type has NO feasibility-passing variant to misuse, and the proposal carries producer provenance (id+version) so the protected gate can bind the §9.2 provenance reference. The stale/missing-model REJECTION itself is the Phase 4 fail-closed policy and cannot be re-implemented in the kernel (composition crate). |
| Protected-path enforcement (cited) | `flexcore-grid-api/src/fail_closed.rs` (the D-P4.08-1 policy table: every trigger routes to REJECTED or HELD — the action vocabulary has NO pass); `flexcore-grid-api/src/model_health.rs` (content hash, staleness bound; only an ACTIVE model is usable); `flexcore-application/src/command_gates.rs` :: `run_feasibility_gate` (a `FailClosed` verdict REJECTS, never passes) — P4.29 invariant 9 row. |
| Tests (SDK slice — NEW this subtask) | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `the_proposal_path_has_no_dispatch_or_feasibility_authority_variant` (exhaustive match: no variant authorizes, passes feasibility, or dispatches; the ADR-010 statement pinned). |
| Tests (protected path, cited) | `flexcore-grid-api/tests/fail_closed.rs` :: `the_fail_closed_action_vocabulary_has_no_pass`, `every_trigger_produces_a_fail_closed_outcome_naming_the_trigger`; `flexcore-grid-api/tests/model_health.rs` :: `only_an_active_model_is_usable`, `a_source_change_after_verification_makes_the_model_stale`; `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_stale_model_aborts_before_the_power_flow`; `flexcore-application/tests/command_gates.rs` :: `a_fail_closed_verdict_rejects_never_passes`, `a_seam_failure_is_fail_closed_never_a_pass` (P4.29). |
| Residual risk / MAP-GAP disposition | The ledger P7.37 step-2 gap ("a plugin result whose §9.2 provenance cites a stale/missing model snapshot; the feasibility gate must reject it — not treat the missing model as a pass") is closed at the layer that owns the decision: the cited Phase 4 suite rejects stale/missing models BEFORE the power flow and the fail-closed vocabulary has no pass. The SDK row adds the structural tripwire (no pass variant on the plugin path). **The full end-to-end composition test** (plugin proposal → feasibility gate → stale-model rejection through one harness) is **MAP-GAP, owning wave: Phase 8** (`31_phase8_handover.md` — the application/plugin integration slice); recorded here, not silently assumed. |
| Ledger-risk note | "Invariant 9 'passing' because the feasibility gate was never invoked for plugin results" — the SDK slice addresses exactly this: the plugin path has no outcome that LOOKS like a feasibility pass, so a skipped gate is visible as an absent Phase-4 evidence reference, never as an SDK-level pass. |

### Invariant 10 — "Site-local safety and protection remain independent of FlexCore."

| | |
|---|---|
| Boundary row (not code) | DOCUMENTED BOUNDARY, not a code test — the P4.29 disposition restated verbatim in meaning: the simulator contains NO modelling of site-local protection devices, relays, or their independence; independence is an INPUT to FlexCore's design (FlexCore commands are supervisory and never a substitute for local protection; §14.5; ADR-012). The SDK adds nothing here and claims nothing here: a plugin's proposal is supervisory data with no protection authority (invariant 7's structural proof applies). |
| Tests | None — by design (a "test" of site-local independence would itself be a protection-grade claim, banned by AGENTS.md §3 rule 3). The boundary statement is pinned as documentation in `crates/flexcore-simulator/src/lib.rs` (*Scope and control status*) and the P4.29 matrix row. |
| Residual risk | None claimable. Any future simulator modelling of local protection is new scope with its own decision record. |

---

## 3. The §14 / §21.4 boundary rows (the task's named additions)

### Boundary row A — the protected command state machine (§14.1–§14.3): no plugin dispatch authority

| | |
|---|---|
| Rule | The command lifecycle `PROPOSED → VALIDATED → FEASIBLE → APPROVED → ISSUED → …` is the protected kernel's; validation, feasibility, and authorization are DISTINCT gates; a plugin proposal enters at PROPOSED and gains no authority from having been proposed (§13.4; ADR-010). |
| SDK enforcement points | `PluginProposalEnvelope` + the closed `PluginPathOutcome` (no dispatch-authorizing variant — `crates/flexcore-kernel/src/plugin_path_protection.rs`); the protected-contract guard at resolver, registry, profile, and path (ADR-005 restated four times); the broker's `()`-returning validation forward. |
| Tests | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `the_proposal_path_has_no_dispatch_or_feasibility_authority_variant`; `crates/flexcore-kernel/tests/plugin_path_protection_p7_36.rs` :: `the_path_outcome_has_no_dispatch_authorizing_variant`; `crates/flexcore-kernel/tests/registry_p7_08.rs` :: `no_factory_registers_under_any_protected_kernel_contract_id`; `apps/plugin-host/tests/sandbox_p7_13.rs` :: `no_permission_id_conveys_dispatch_authority_unbuildable_not_discouraged`. |
| Residual risk | The lifecycle itself is domain-owned (`flexcore-domain/src/command_lifecycle.rs`, frozen-edge oracle, P4.33 matrix); the SDK cannot enforce what it does not own — cited, not duplicated. |

### Boundary row B — the control-authority mode lock (§21.4): mode is a deployment/per-site setting, never a plugin or profile selection

| | |
|---|---|
| Rule | "Control-authority mode is a deployment and per-site setting, not something a plugin can select" (§21.4; §5.8). |
| SDK enforcement points | The profile data lock (P7.15): `ControlAuthorityPosture::parse_from_profile_data` accepts EXACTLY `simulation-shadow`; any other §21.4 mode in profile data is `CONTROL_AUTHORITY_MODE_FORBIDDEN`; the P7.16 recomposition boundary refuses a posture change handed in from non-profile code (defence in depth); the never-granted set keeps `dispatch.authorize`/`dispatch.issue` off every plugin surface. |
| Tests | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `the_mode_lock_holds_profile_data_cannot_select_control_authority`; `crates/flexcore-kernel/tests/profile_p7_15.rs` :: `any_control_authority_value_other_than_simulation_shadow_is_a_named_error`, `the_posture_enum_has_the_four_21_4_modes_but_data_reaches_only_simulation_shadow`; `crates/flexcore-kernel/tests/recomposition_p7_16.rs` :: `a_switch_that_would_change_the_control_authority_posture_is_refused`; `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` :: `research_profile_context_refuses_dispatch_permissions`. |
| Residual risk | The DEPLOYMENT's own mode configuration (per-site, operator-approved from v1.0.0) is composition/operator scope; the SDK guarantees only that no plugin, profile, or composition switch can raise it. |

### Boundary row C — the release ladder: the entire 0.x line is simulation-only

| | |
|---|---|
| Rule | Field write authority begins at v1.0.0 with operator approval (AGENTS.md §4 release ladder; §3 rule 5). |
| SDK enforcement points | The plugin SDK has no write path to grant: no network/file/clock ambient capability (host P7.11/P7.13), the P4.16 adapter guard refuses every non-simulator dispatch target (cited), and every module statement carries the simulation-only posture. |
| Tests | `apps/plugin-host/tests/sandbox_p7_13.rs` :: `every_ambient_denial_is_audited`; `apps/plugin-host/tests/no_dylib_p7_35.rs` (the loading-surface scans); the P4.29 invariant-6/7 suites (cited above). |
| Residual risk | The v1.0.0 promotion gate is organizational (§22.3 release gates), not an SDK mechanism. |

---

## 4. Coverage ledger (the P7.32 exit-check input)

| §17.5 row | SDK row status | Proving tests exist? |
|---|---|---|
| 1 asset/site identity + current context | SDK identity-binding slice only; asset checks cited to Phase 3/4 | Yes (cited both layers) |
| 2 capability validation | SDK never-granted slice + cited application gate | Yes |
| 3 network feasibility evidence | SDK structural slice (no pass variant); gate cited to Phase 4 | Yes |
| 4 authorization | SDK structural slice (no verdict object); decision cited to Phase 4 | Yes |
| 5 stale/expired | SDK window/sequence/clock slice live | Yes |
| 6 unapproved adapter/target | SDK no-network + protected-validator-target slice | Yes |
| 7 no direct physical command from a plugin | SDK structural proof (four guard layers) | Yes |
| 8 cross-tenant | SDK envelope slice (P7.39, new) + platform RLS cited | Yes |
| 9 stale model ≠ pass | SDK structural slice (new matrix test) + Phase 4 fail-closed suite cited; **end-to-end composition test = MAP-GAP, owning wave: Phase 8** | Yes (slice) / GAP recorded |
| 10 site-local independence | Boundary row, no code test by design | N/A (documented boundary) |
| §14/§21.4 boundary rows A/B/C | Mode lock live; ADR-010 structural live; simulation-only pinned | Yes |

**P7.32 must not close the phase while the invariant-9 end-to-end MAP-GAP
row is open** (ledger P7.37 step 4). No other row is open.

## 5. Honesty statements

- Every test named above is WRITTEN, UNRUN until the human build/test
  pass. Nothing in this map claims a run result.
- The map cites design sections ONLY where it also cites the test that
  proves them (the ledger's named risk: "a map that cites design sections
  instead of test runs" — every row carries test file + fn names).
- No safety or certification claim is made (AGENTS.md §3 rules 2/5; the
  report's own §17.5 closing sentence is quoted in §1).
