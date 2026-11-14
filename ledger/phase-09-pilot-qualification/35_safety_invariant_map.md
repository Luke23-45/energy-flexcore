# P9.35 — §17.5 safety-invariant map for the pilot (WRITTEN-UNRUN)

Date: 2026-11-14. Subtask: P9.35 (`ledger_phase_9.md` §P9.35). Owner role:
Security (with power systems). Report trace: §23 Stage 9; §17.5 (invariants
1–10, quoted verbatim per row); §14.3; §12.5; §13.3 (the finite-test-set
discipline). Precedent: the Phase 7 map
(`ledger/phase-07-plugin-sdk/37_safety_invariant_map.md`) — this record is
its PILOT version: same honesty rules, different scope (the pilot deployment
picture across the edge agent, the central platform layers, and the pilot
subtasks that prove each invariant live).

**Run-status honesty (AGENTS.md §3 rule 7; ENTRY_NOTE boundaries 1 and 6):**
this record is a MAP, not a test result. Every test cited below is
WRITTEN-UNRUN until the human build/test pass executes it; every pilot live
test named (P9.14/P9.15/P9.17/P9.18/P9.19/P9.20/P9.36) is UNRUN /
NOT-SCHEDULED — those runs are human/organization gate inputs and no agent
executed any of them. Nothing here claims a test passed, a site exists, or a
pilot ran. A row whose only evidence is a design statement is marked OPEN,
never covered by prose.

---

## 1. How to read this map

Each of the ten §17.5 rules is recorded with FIVE things:

1. **The rule, verbatim** from report §17.5 (never paraphrased into a new
   meaning). §17.5's own closing sentence governs every row: "These are
   system requirements, not a certification claim."
2. **Where the rule is enforced in THIS codebase** — file + symbol, cited
   (verified to exist by reading on 2026-11-14). The pilot-relevant layers
   are the edge agent (`crates/edge-agent`), the application/grid-api/domain
   layers (central), and the plugin kernel (structural, Phase 7).
3. **Pilot-phase enforcement status** — `code-enforced` (the enforcement is
   in committed code) vs `deployment-configuration` (the enforcement is a
   per-site setting/provisioning act, not code) vs `human/organizational`
   (the enforcement is a procedure the pilot must run).
4. **Verification evidence state** — every state is WRITTEN-UNRUN unless
   explicitly marked NOT-YET-BUILT (the proving artifact does not exist yet)
   or NOT-RUNNABLE-HERE (it needs pilot infrastructure no agent can touch).
5. **The pilot proving subtask** — the P9.xx that must run the invariant
   live at the deployment, and its current schedule state.

**The §13.3 discipline (stated once here, governs every row):** the report's
own rule — "A finite test set with zero observed constraint violations is
not a universal safety guarantee." Every proving test in this map is a
finite test set; zero observed violations in any of them is a TEST OUTCOME,
not a guarantee that no input outside the set can violate the invariant.
No row in this map may be read as a safety, certification, or
protection-grade claim (AGENTS.md §3 rules 2/3/5; FlexCore is supervisory
orchestration only).

**Pilot-scope honesty:** Phase 9 qualifies ONE named customer deployment at
named sites for operator-approved control AT MOST (ENTRY_NOTE boundary 3).
No row grants field write authority beyond the §21.4 ladder, and the pilot
scope/customer/site themselves are the P9.01 freeze (recorded; the named
customer and sites are P9.01's to name — this map never invents them).

---

## 2. The Phase 7 invariant-9 map-gap — carried honestly, still OPEN

The Phase 7 map (`37_safety_invariant_map.md`, invariant 9 row, "Residual
risk / MAP-GAP disposition") recorded one MAP-GAP: **the full end-to-end
composition test** — plugin proposal → feasibility gate → stale-model
rejection through one harness — with "owning wave: Phase 8"
(`31_phase8_handover.md`, the application/plugin integration slice).

**Disposition after checking the Phase 8 records (2026-11-14): NOT CLOSED.**
What was checked: every Phase 8 record file
(`ledger/phase-08-advanced-coordination/01…39`, plus `CHECKLIST.md`,
`CONTEXT.md`, `ledger_phase_8.md`) was searched for the invariant map, the
map-gap, and the invariant-9 composition test. Findings:

- No Phase 8 record mentions P7.37, the safety-invariant map, or the
  invariant-9 end-to-end composition test (grep over the whole directory
  returns only unrelated uses of the word "invariant" — property-test
  invariants in `37_research_tests.md` and the §17.5 pointers in
  `CONTEXT.md`/`34_proposal_windows.md`).
- Phase 8 DID build an independent validation harness for RESEARCH
  proposals (`crates/flexcore-research/src/validation.rs` — capability,
  power-flow revalidation seam, SOC/ramp/duration, command invariants;
  `37_research_tests.md`), and its stale/superseded-snapshot stage
  (`SNAPSHOT_SUPERSEDED`/`SNAPSHOT_UNKNOWN`) is the invariant-9 shape for
  the research path. That is a DIFFERENT composition from the one the gap
  names: it validates research proposals, not plugin proposals flowing
  through the kernel plugin path into the feasibility gate. The gap as
  written — plugin proposal → feasibility gate → stale-model rejection in
  one harness — has no owning test anywhere in the tree.
- The Phase 8 qualification and exit-gate records (`29_qualification_record.md`,
  `30_retrospective_exit_gate.md`) do not carry the map-gap either.

**Recorded disposition: the invariant-9 end-to-end composition map-gap
remains OPEN.** Owner: the application/plugin integration owner (the
`flexcore-application` ↔ `flexcore-kernel` slice — the same owner the
Phase 7 handover named; it was not consumed by Phase 8). Carried to: this
map's invariant-9 row (below), the P9.27 residual-risk register (named
there), and Phase 10. This subtask does NOT close it by prose, does NOT
invent the composition test in another owner's crate, and does NOT claim
the Phase 8 research harness covers it.

---

## 3. The invariant rows

### Invariant 1 — "No command without a valid asset/site identity and current command context."

| | |
|---|---|
| Verbatim rule | "No command without a valid asset/site identity and current command context." (report §17.5 item 1) |
| Enforcement — edge (code-enforced) | `crates/edge-agent/src/command_verify.rs` :: `CommandVerifier::verify` check 1 — the signature binds the command id into the canonical binding (`canonical_command_binding`; `check_signature`: wrong-signer and payload-binding-mismatch both refuse); `CommandVerifyError::InvalidCommand` refuses an empty identity, a non-finite value, an inverted validity window; check 4b `check_scope` refuses a command whose asset has no scope entry. Gate layer: `crates/edge-agent/src/command_path.rs` :: `CommandReceiver::receive` + `CommandRefusal::AdapterNotAllowlisted`; `crates/edge-agent/src/policy.rs` :: `SitePolicyEngine::admit_command` (`PolicyViolation::UnknownAdapter` — fail-closed, never defaulted). Site identity: `crates/edge-agent/src/enrollment.rs` :: `DeviceBinding` (tenant+site binding validated at enrollment; `DeviceCredentialRecord::validate`); the P9.33 ceremony refuses a wrong-site/wrong-identity enrollment (test `crates/edge-agent/tests/enrollment_ceremony_p9_33.rs` :: `the_wrong_site_and_wrong_identity_failed_enrollment_cases_are_refused_and_audited`, WRITTEN-UNRUN). |
| Enforcement — central (code-enforced) | `crates/flexcore-application/src/command_gates.rs` :: `create_proposed_command` + `run_validation_gate` (asset exists in the caller's tenant; unknown/foreign assets reject without leaking existence — the P4.29 invariant-1 row, cited not re-verified); `crates/flexcore-domain/src/tenant_context.rs` :: `TenantContext::from_authenticated_claims`. |
| Pilot-phase enforcement status | code-enforced (edge + central) + deployment-configuration (the site allowlist, the D6 record reference, and the credential record are per-site provisioning inputs — P9.05/P9.06/P9.09). |
| Proving tests (WRITTEN-UNRUN) | `crates/edge-agent/tests/command_verify_p6_15.rs` :: `unsigned_forged_and_wrong_signer_commands_reject_with_the_signature_reason`, `out_of_scope_limits_quantities_and_assets_reject_with_the_scope_reason`; `crates/edge-agent/tests/pilot_rejection_proof_p9_31.rs` :: `forged_but_otherwise_valid_payloads_reject_with_the_signature_reason`; central suites cited from the P4.29 matrix via the Phase 7 map (row 1). |
| Pilot live proving (NOT-RUNNABLE-HERE) | THE LEDGER STEP-2 GAP — this subtask's step 2 names two pilot tests: (a) a command carrying a WRONG-SITE identity, (b) a command built on a SUPERSEDED context. Neither exists as a pilot run today. Honest mapping: (a) at the edge, a wrong-site identity manifests as an allowlist/scope refusal (`CommandRefusal::AdapterNotAllowlisted`, `check_scope`) or an enrollment-binding refusal (`DeviceBinding`); the LIVE wrong-site-identity injection belongs to the P9.36 drill scenario (its §3 case table) and P9.19's dry run. (b) a superseded-context command at the edge manifests as `CommandVerifyError::InvalidAuthorization` (self-contradictory/stale authorization reference, `CommandAuthorization::validate` in `command_verify.rs`) or the expiry check; the LIVE superseded-context injection belongs to P9.19 step 2 (a candidate that fails one gate is not passed by another). **Neither pilot test is scheduled; both are recorded OPEN with owner = the P9.19/P9.36 execution owners (human gates). P9.28 must not approve a site while either is unrun.** |
| Residual risk | The edge verifies signature/expiry/sequence/allowlist/scope but does NOT re-decide asset identity against the registry — a wrong-asset command that is validly signed, in-window, in-sequence, and allowlisted would pass the edge checks; the asset-vs-tenant identity decision is central (`run_validation_gate`). Recorded, not hidden. |

### Invariant 2 — "No command without capability validation."

| | |
|---|---|
| Verbatim rule | "No command without capability validation." (report §17.5 item 2) |
| Enforcement — central (code-enforced) | `crates/flexcore-application/src/command_gates.rs` :: `run_validation_gate` (capability model, ramp vs declared capability) + per-dimension verdicts in `crates/flexcore-application/src/command_record.rs` (the P4.29 invariant-2 row). |
| Enforcement — edge (code-enforced, the slice the edge owns) | `crates/edge-agent/src/command_verify.rs` :: `check_scope` — the command's quantity must be an allowed quantity and its value must sit inside the approved limits of the command's `CommandAuthorization` (`CommandVerificationRejection::Scope`); `CommandAuthorization::validate` refuses an incomplete or self-contradictory authorization reference. The edge bounds commands to the AUTHORIZED limits; the asset-capability comparison itself is central (the edge has no capability models — recorded honestly, same split as the Phase 7 map). |
| Pilot-phase enforcement status | code-enforced (central capability gate; edge scope check). |
| Proving tests (WRITTEN-UNRUN) | `crates/edge-agent/tests/command_verify_p6_15.rs` :: `out_of_scope_limits_quantities_and_assets_reject_with_the_scope_reason`; `crates/edge-agent/tests/pilot_rejection_proof_p9_31.rs` (the scope rejections inside the attack matrix); central: `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_capability_exceedance_is_rejected_naming_the_asset` and `flexcore-application/tests/command_gates.rs` :: `a_missing_capability_model_and_undeclared_quantities_are_rejected` (P4.29, cited). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.19 step 2 (a candidate fails validation independently) — NOT SCHEDULED, human gate. |
| Residual risk | Edge scope limits are as-approved-at-authorization, not as-measured-at-asset; a stale capability model central-side is the §17.5 invariant-9 shape (fail-closed there). |

### Invariant 3 — "No command without required network feasibility evidence for the declared operating profile."

| | |
|---|---|
| Verbatim rule | "No command without required network feasibility evidence for the declared operating profile." (report §17.5 item 3) |
| Enforcement — central (code-enforced) | `crates/flexcore-grid-api/src/fail_closed.rs` (the D-P4.08-1 policy table — the action vocabulary has NO pass) + `crates/flexcore-grid-api/src/model_health.rs` (content hash, staleness bound; only an ACTIVE model is usable — `latest_active`, `verify_content_hash`) + `crates/flexcore-grid-api/src/feasibility.rs`; `crates/flexcore-application/src/command_gates.rs` :: `run_feasibility_gate` + `ensure_command_issueable` (the evidence's model hash must equal the ACTIVE model hash before ISSUED). |
| Enforcement — edge | NONE, structurally: the edge agent has NO feasibility surface. The edge verifies a command it was SENT (§18.2); feasibility is decided centrally BEFORE issue (§12.5). No edge module produces or consumes a feasibility verdict — absence is structural (`crates/edge-agent/src/command_forward.rs` :: `FailClosedCommandForwarder` forwards only a `VerifiedCommand`; nothing else exists to forward). |
| Pilot-phase enforcement status | code-enforced (central) + NOT-YET-BUILT pilot infrastructure: the pilot grid model itself (P9.03's model is a frozen design record; the validated model version and its residuals come from the human validation run) and the declared operating profile per site (P9.01/P9.26). |
| Proving tests (WRITTEN-UNRUN) | `flexcore-grid-api/tests/fail_closed.rs`, `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_stale_model_aborts_before_the_power_flow`, `flexcore-application/tests/command_gates.rs` :: `feasibility_evidence_binds_the_model_hash_and_a_model_change_blocks_issue` (all P4.29, cited from the Phase 7 map — they exist in the tree, UNRUN in this loop). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.03 (model validation residuals against measured points) + P9.19 (the dry run's feasible/infeasible gate separation) — both NOT SCHEDULED, human gates. THE LEDGER STEP-3 CHECK (invariant 3 must have a LIVE test, not a design statement): until P9.03's validation run and P9.19's dry run execute, this row has code-level suites only — recorded OPEN on exactly that ground, not papered over. |
| Residual risk | SOCP relaxation caution (§12.5): a feasibility pass is relaxation-based evidence, not an AC guarantee — the pilot's thresholds must be set in advance (P9.03 step 4), never fitted after. |

### Invariant 4 — "No command without authorization."

| | |
|---|---|
| Verbatim rule | "No command without authorization." (report §17.5 item 4) |
| Enforcement — central (code-enforced) | `crates/flexcore-application/src/command_gates.rs` :: `record_command_approval` (the only APPROVED producer); `crates/flexcore-authz` deny-by-default policies (P4.29 invariant-4 row). |
| Enforcement — edge (code-enforced) | `crates/edge-agent/src/command_verify.rs` :: check 4b — every command carries a `CommandAuthorization` (authorization reference, approved scope/limits/expiry; `validate` refuses an incomplete/self-contradictory reference; the authorization window is checked inside the expiry check against the INJECTED clock); `crates/edge-agent/src/command_path.rs` :: `CommandPathRuntime::evaluate` — the four-condition enablement gate (config flag + SIGNED enablement record + token match + allowlist + D6 record reference; `GateRefusal` codes) with NO public setter and no `&mut self` path from disabled to enabled (the structural no-plugin-enable rule). |
| Pilot-phase enforcement status | code-enforced (edge + central) + human/organizational (the P9.08 role matrix, IdP integration, separation of duties, and step-up are deployment/procedure controls the code cannot supply — P9.08 is a NOT-YET-BUILT pilot artifact at the time of writing this map: its record is being produced by the concurrent wave). |
| Proving tests (WRITTEN-UNRUN) | `crates/edge-agent/tests/command_verify_p6_15.rs` (scope/authorization rejections; the expiry-window-of-authorization cases); `crates/edge-agent/tests/command_path_p6_14.rs` (the gate negatives); central `flexcore-application/tests/rejection_unauthorized.rs` (P4.23, cited). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.08 step 6 (negative role-matrix tests at the pilot) + P9.19 (the dry run's unauthorized-candidate gate) — NOT SCHEDULED, human gates. |
| Residual risk | The edge trusts the authorization reference PRESENTED with the command (it is bound into the signature); the edge cannot detect a legitimately-signed authorization that was revoked central-side after issue — revocation propagation is the P9.09 procedure (human). |

### Invariant 5 — "No stale or expired command."

| | |
|---|---|
| Verbatim rule | "No stale or expired command." (report §17.5 item 5) |
| Enforcement — edge (code-enforced) | `crates/edge-agent/src/command_verify.rs` :: `verify` step 2 — the validity window against the INJECTED edge clock WITH the recorded clock-quality check: any quality other than `Synchronized` (including `Unknown`) FAILS CLOSED (`crates/edge-agent/src/clock_quality.rs`, the P6.31 seam); the sequence check runs LAST so a refused command never advances the window (the P4.28 `SequenceGuard` rule); `crates/edge-agent/src/command_verify.rs` :: `FileSequenceCheckpoint` — the per-channel window survives an edge restart (a replay after restart is still `duplicate`). |
| Enforcement — central (code-enforced) | `crates/flexcore-domain/src/command_lifecycle.rs` :: `record_expiry`; `crates/flexcore-application/src/command_expiry.rs` (the sweep); `crates/flexcore-application/src/envelope_staleness.rs`. |
| Pilot-phase enforcement status | code-enforced (edge + central) + deployment-configuration (the site's actual clock synchronization is a P9.18 measurement, not a code fact). |
| Proving tests (WRITTEN-UNRUN) | `crates/edge-agent/tests/command_verify_p6_15.rs` :: `an_aged_or_not_yet_valid_command_rejects_with_the_expiry_reason`, `a_degraded_or_unknown_edge_clock_fails_the_expiry_check_closed`, `duplicate_and_out_of_sequence_are_distinct_rejections`, `sequence_state_survives_an_edge_restart_via_the_checkpoint`, `a_checkpoint_store_failure_refuses_the_command_fail_closed`; `crates/edge-agent/tests/pilot_rejection_proof_p9_31.rs` :: `exactly_at_expiry_rejects_and_one_millis_before_passes`, `a_replayed_captured_command_is_rejected_and_survives_an_edge_restart`, `out_of_sequence_gaps_behind_reject_while_a_head_gap_is_admitted_by_design`; `crates/edge-agent/tests/clock_quality_p6_31.rs`. |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.18 step 2 (measured clock offset within the bound the command expiry assumes) — NOT SCHEDULED, human gate. |
| Residual risk | Recorded head-gap design: a sequence AHEAD of the last accepted is admitted (monotonicity, not density — P9.31 record finding 2); density enforcement is the recorded Phase 7 follow-up. The edge cannot detect a lying clock implementation — P9.18 measures the real one. |

### Invariant 6 — "No unapproved protocol adapter or target."

| | |
|---|---|
| Verbatim rule | "No unapproved protocol adapter or target." (report §17.5 item 6) |
| Enforcement — edge (code-enforced + deployment-configuration) | `crates/edge-agent/src/command_verify.rs` :: check 4a — the target adapter must be on the site allowlist (`CommandVerificationRejection::Allowlist`); `crates/edge-agent/src/command_forward.rs` :: `SiteCommandAllowlist` + `CommandForwardingEngine::forward_verified` (its own SEPARATE allowlist + policy re-checks before any adapter contact; `ForwardingRefusal` codes); `crates/edge-agent/src/policy.rs` :: `SitePolicyEngine::admit_command` (`UnknownAdapter` / `AdapterDisabled` / rate limit / operating window); `crates/edge-agent/src/command_path.rs` :: the P6.14 gate (allowlist is also an enablement condition — an empty allowlist means the path never enables, `GateRefusal::AllowlistEmpty`). Network shape: `crates/edge-agent/src/uplink_mtls.rs` (outbound-only; NO listen/bind/accept anywhere — enforced by `crates/edge-agent/tests/no_inbound_listener.rs`). |
| Enforcement — central (code-enforced) | `crates/flexcore-application/src/dispatch_simulator.rs` :: `configure` — the D-P4.16-6 hard guard (only the simulator target accepted). |
| Pilot-phase enforcement status | code-enforced (edge allowlist/policy; central target guard) + human/organizational (WHICH adapters a site may run is the P9.04 integration design + P9.26 control-scope document — an approval workflow, not code; D5 position is agent-undecided per ENTRY_NOTE boundary 2). |
| Proving tests (WRITTEN-UNRUN) | `crates/edge-agent/tests/command_verify_p6_15.rs` :: `a_non_allowlisted_target_rejects_with_the_allowlist_reason`; `crates/edge-agent/tests/policy_health_p6_17.rs`; `crates/edge-agent/tests/adapter_acl_and_registry.rs`; `crates/edge-agent/tests/no_inbound_listener.rs`; central `flexcore-application/tests/dispatch_simulator.rs` :: `the_target_guard_refuses_every_non_simulator_kind` (cited). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.04 (the per-adapter design with named versions/profiles) + P9.26 (the approved control-scope configuration) + P9.37 (conformance suites executed in the pilot configuration) — P9.04's record exists as a design; the approval workflow and P9.37 runs are NOT SCHEDULED, human gates. |
| Residual risk | The code guarantees a command cannot reach a NON-allowlisted target; it cannot guarantee every allowlisted target IS approved — that is exactly the P9.26 control-scope document's job. |

### Invariant 7 — "No direct physical command from an optimizer, GNN, MARL, LLM, or general plugin."

| | |
|---|---|
| Verbatim rule | "No direct physical command from an optimizer, GNN, MARL, LLM, or general plugin." (report §17.5 item 7) |
| Enforcement — structural (code-enforced, four layers, cited from the Phase 7 map row 7) | Kernel: `crates/flexcore-kernel/src/plugin_path_protection.rs` — the protected-contract guard (ADR-005) at resolver/registry/profile/path, and the closed `PluginPathOutcome` with NO dispatch-authorizing variant (ADR-010; the exhaustive-match tests fail to compile if a variant is added — the tripwire). Host: `apps/plugin-host` broker/enforcement re-refuse the never-granted set (`dispatch.authorize`, `dispatch.issue`). Domain: `crates/flexcore-domain/src/command_lifecycle.rs` — the frozen-edge oracle (no PROPOSED → ISSUED edge). Edge: `crates/edge-agent/src/command_forward.rs` :: `FailClosedCommandForwarder` — the ONLY route to an adapter is a `VerifiedCommand` through `forward_verified`; a research/optimizer output has no such shape. |
| Pilot-phase enforcement status | code-enforced (structural) + human/organizational (the research modules in the pilot shadow are the P9.17/P9.38 human-scoped activities; §13.3 bars live MARL device commands). |
| Proving tests (WRITTEN-UNRUN) | `crates/flexcore-kernel/tests/safety_invariant_p7_37.rs` :: `the_proposal_path_has_no_dispatch_or_feasibility_authority_variant`, `protected_kernel_contract_claims_are_refused_at_resolver_and_path`; `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` :: `a_proposed_dispatch_gains_no_authority_adr010`; `flexcore-domain/tests/transition_matrix.rs` :: `no_edge_outside_the_frozen_table_exists`; edge `crates/edge-agent/tests/command_forward_p6_16.rs` (cited; the P6.16 forwarder negatives). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.19 step 3 + its verification method ("confirm no ISSUED state exists for any dry-run command ID") — NOT SCHEDULED, human gate. |
| Residual risk | The proof is structural, not behavioural — a future type with dispatch authority needs its own decision record and refusal tests (the Phase 7 map's residual, unchanged). |

### Invariant 8 — "No cross-tenant command or telemetry access."

| | |
|---|---|
| Verbatim rule | "No cross-tenant command or telemetry access." (report §17.5 item 8) |
| Enforcement — central (code-enforced) | `TenantContext::from_authenticated_claims` (authenticated claims only, never request fields — §16.2); application authorization + PostgreSQL RLS as defence in depth (the RLS FORCE/non-owner-role configuration is DEPLOYMENT configuration — P9.10 verifies it by test, the code cannot). Kernel SDK slice: `crates/flexcore-kernel/src/tenant_scope.rs` :: `TenantScope::from_authenticated` (host-stamped, never plugin-supplied) + the `ScopedRequestEnvelope` binding (P7.39). |
| Enforcement — edge (code-enforced) | `crates/edge-agent/src/enrollment.rs` :: `DeviceBinding` — the edge credential is bound to ONE tenant+site at enrollment; `DeviceCredentialRecord::validate` refuses an incomplete binding; the P9.33 ceremony refuses re-enrollment across tenants (`crates/edge-agent/tests/enrollment_ceremony_p9_33.rs` :: `the_wrong_site_and_wrong_identity_failed_enrollment_cases_are_refused_and_audited`, WRITTEN-UNRUN). |
| Pilot-phase enforcement status | code-enforced (central; edge binding) + deployment-configuration (RLS FORCE ROW LEVEL SECURITY and non-owner service roles — §16.2, [R25]). |
| Proving tests (WRITTEN-UNRUN) | `crates/edge-agent/tests/tenant_isolation_p6_21.rs`; `crates/flexcore-kernel/tests/tenant_scope_p7_39.rs` :: `cross_tenant_attempts_are_refused_and_audited_end_to_end`; `flexcore-authz/tests/security_authz.rs` :: `cross_tenant_scope_denies_even_for_platform_admin` (cited). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.10 step 3 (the tenancy negative test in the pilot environment) — NOT SCHEDULED, human gate. |
| Residual risk | A misconfigured RLS deployment has only the envelope + application layers — recorded in the Phase 7 map and unchanged; P9.10's test is the pilot's evidence. |

### Invariant 9 — "A missing/uncertain/stale model is not interpreted as a passing feasibility check."

| | |
|---|---|
| Verbatim rule | "A missing/uncertain/stale model is not interpreted as a passing feasibility check." (report §17.5 item 9) |
| Enforcement — central (code-enforced) | `crates/flexcore-grid-api/src/fail_closed.rs` (every trigger routes to REJECTED or HELD — the vocabulary has NO pass) + `crates/flexcore-grid-api/src/model_health.rs` (content hash; staleness bound; `latest_active` — only an ACTIVE model is usable); `crates/flexcore-application/src/command_gates.rs` :: `run_feasibility_gate` (a `FailClosed` verdict REJECTS, never passes). Research-path analogue (Phase 8): `crates/flexcore-research/src/validation.rs` Stage 5 — `SNAPSHOT_SUPERSEDED`/`SNAPSHOT_UNKNOWN` ("unknown is never fresh"); the physical-revalidation seam failure is fail-closed (`PHYSICAL_REVALIDATION_SEAM_FAILED`, never a pass). |
| Enforcement — edge | NONE, structurally (same as invariant 3 — the edge has no feasibility surface; nothing at the edge can LOOK like a feasibility pass). |
| Pilot-phase enforcement status | code-enforced (central) + **ONE OPEN MAP-GAP carried from Phase 7 (§2 of this record): the end-to-end composition test (plugin proposal → feasibility gate → stale-model rejection through one harness) was never built; the Phase 8 records do not close it. Recorded OPEN, owner = the application/plugin integration owner; carried to P9.27 and Phase 10.** |
| Proving tests (WRITTEN-UNRUN) | `flexcore-grid-api/tests/fail_closed.rs` :: `the_fail_closed_action_vocabulary_has_no_pass`, `every_trigger_produces_a_fail_closed_outcome_naming_the_trigger`; `flexcore-grid-api/tests/model_health.rs` :: `only_an_active_model_is_usable`, `a_source_change_after_verification_makes_the_model_stale`; `flexcore-grid-api/tests/feasibility_gate.rs` :: `a_stale_model_aborts_before_the_power_flow`; `flexcore-application/tests/command_gates.rs` :: `a_fail_closed_verdict_rejects_never_passes`, `a_seam_failure_is_fail_closed_never_a_pass` (all P4.29, cited); research-path `flexcore-research` suite (P8.05, cited). |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.18 step 5 (inject a stale feed and a missing feed; the system must hold/reject, never infer — the ledger's own invariant-9 wording) — NOT SCHEDULED, human gate. |
| Residual risk | THE OPEN MAP-GAP itself (§2). Until the composition test exists, the invariant-9 chain plugin→feasibility-gate is proven at each LAYER (structural SDK tripwire + fail-closed gate suites) but not in ONE end-to-end run — an honest, recorded limitation, not a closed one. |

### Invariant 10 — "Site-local safety and protection remain independent of FlexCore."

| | |
|---|---|
| Verbatim rule | "Site-local safety and protection remain independent of FlexCore." (report §17.5 item 10) |
| Enforcement | DOCUMENTED BOUNDARY, not a code test — the P4.29 disposition restated in the Phase 7 map row 10 applies verbatim: the simulator contains NO modelling of site-local protection devices, and a "test" of local-protection independence would itself be a protection-grade claim (banned, AGENTS.md §3 rule 3). What the CODE does supply at the edge is the fallback discipline: `crates/edge-agent/src/command_forward.rs` :: `FallbackPolicyConfig` / `FallbackPolicyEntry` / `FallbackTrigger` (the fallback is defined PER ASSET/SITE — never a universal "set power to zero"; §14.4), `OperatorReview` (a fallback policy requires a recorded operator review), and `FailClosedCommandForwarder` (FlexCore supervises; it is never a substitute for local control). §17.4's central-outage row: site-local fallback and autonomous local EMS, no dependence on cloud for protection. |
| Pilot-phase enforcement status | human/organizational + code-enforced (the per-asset fallback policy structure only). The INDEPENDENCE itself is an input to the pilot: P9.13 commissioning safety rules, P9.20's per-asset fallback register reviewed with the equipment/site operator, and the drills. |
| Proving tests (WRITTEN-UNRUN) | None by design (the Phase 7 row's disposition, unchanged). The fallback MECHANICS have suites: `crates/edge-agent/tests/command_forward_p6_16.rs` (fallback activation/expiry mid-flight), WRITTEN-UNRUN. |
| Pilot live proving (NOT-RUNNABLE-HERE) | P9.20 (every degraded mode drilled; the central-outage drill witnessed) + P9.13 (commissioning safety rules) — NOT SCHEDULED, human gates. P9.20's acceptance criterion (c) — no drill shows a central outage disabling local protection — is the pilot's invariant-10 evidence; until those drills run, this row is OPEN on exactly that ground. |
| Residual risk | None claimable by code: if a site's local protection is NOT independent, no FlexCore mechanism fixes that — it is a site-acceptance fact (P9.28), not a software property. |

---

## 4. Coverage ledger

| §17.5 row | Enforcement in this codebase | Pilot status | Evidence state | Open? |
|---|---|---|---|---|
| 1 identity + current context | edge verify/scope/allowlist + central validation gate | code-enforced + per-site config | WRITTEN-UNRUN; the two ledger-step-2 pilot tests (wrong-site identity, superseded context) NOT SCHEDULED | OPEN on the pilot-run ground (P9.19/P9.36) |
| 2 capability validation | central gate + edge scope check | code-enforced | WRITTEN-UNRUN | No (pilot run P9.19 pending) |
| 3 feasibility evidence | central fail-closed chain; edge has none (structural) | code-enforced + NOT-YET-BUILT pilot model/profile | WRITTEN-UNRUN; P9.03/P9.19 runs NOT SCHEDULED | OPEN on the pilot-run ground |
| 4 authorization | central approval + edge four-check/enablement gate | code-enforced + P9.08 procedures (concurrent wave) | WRITTEN-UNRUN | No (pilot run P9.08/P9.19 pending) |
| 5 stale/expired | edge window+clock-quality+checkpoint; central expiry | code-enforced + P9.18 measurement | WRITTEN-UNRUN | No (pilot run P9.18 pending) |
| 6 unapproved adapter/target | edge allowlist/policy/gate + central target guard | code-enforced + P9.04/P9.26 approval workflow | WRITTEN-UNRUN | No (pilot approval pending) |
| 7 no plugin dispatch | ADR-010 structural four layers + frozen-edge oracle | code-enforced (structural) | WRITTEN-UNRUN | No (pilot run P9.19 pending) |
| 8 cross-tenant | tenant context + RLS (deploy config) + edge DeviceBinding | code-enforced + deployment config | WRITTEN-UNRUN | No (pilot test P9.10 pending) |
| 9 stale model ≠ pass | central fail-closed + research analogue | code-enforced | WRITTEN-UNRUN | **YES — the Phase 7 map-gap composition test: OPEN, owner named (§2)** |
| 10 site-local independence | documented boundary + per-asset fallback structure | human/organizational | NOT-RUNNABLE-HERE (by design, no code test) | OPEN until P9.20 drills run |

**P9.28 linkage (ledger step 4):** the file location for the acceptance pack
is this record; P9.28 must not approve a site while any row in §4 is open.
Rows open at the time of writing: 1 (pilot-run ground), 3 (pilot-run
ground), 9 (the composition map-gap), 10 (drill ground). Closing them
requires the HUMAN runs, not edits to this map.

---

## 5. Invariants depending on NOT-YET-BUILT pilot infrastructure (explicit list)

Code-enforced today (no pilot infrastructure needed): the edge four-check
verifier, the P6.14 enablement gate, the site policy engine, the P9.31
attack-proof surface, the audit buffer/chain, the enrollment ceremony
structure, the update gate — all cited above.

NOT-YET-BUILT (the invariant's PILOT evidence depends on them):

| Invariant | Missing infrastructure | Owning subtask |
|---|---|---|
| 1, 4 | Customer IdP integration, pilot role matrix, step-up enforcement | P9.08 (in production by the concurrent wave; its negative tests are human runs) |
| 1, 4, 5, 6, 8 | The named pilot site itself: hardware, commissioning, credential provisioning, allowlist/D6 inputs per site | P9.13 (plan, human), P9.09 (secrets/PKI), P9.06 (D6 closure — human signature) |
| 3 | The frozen, validated pilot grid model + declared operating profile | P9.03 (record exists; the validation RUN is human) |
| 3, 5, 9 | HIL platform access and calibration evidence | P9.15 (human; CHIL/PHIL scope, §19.2 order) |
| 5 | Real clock-sync measurement at the site | P9.18 (human) |
| 6 | Adapter approval workflow and control-scope configuration | P9.26 (design; approval is human) |
| 10 | Per-asset fallback policies reviewed with the site operator; the drills | P9.20 (human) |
| ALL | The cryptographic replacements the code honestly lacks: a real MAC/signature for `edge-sig:` (P6.15 deployment requirement), a CSPRNG for ceremony nonces (P9.33), SHA-256+ for update verification (P9.34), the platform-side §14.3 idempotency registry (Phase 7 follow-up), RLS deployment configuration (§16.2) | P9.09 + the recorded Phase 7 follow-ups |

---

## 6. Reviewer-verification procedure (human)

1. Trace each §3 row's citations: every file+symbol named must exist
   (`grep -n "<symbol>" <file>`); any dead citation is a map defect.
2. Run the WRITTEN-UNRUN suites (the human build/test pass):
   `cargo test -p flexcore-edge-agent` and the central suites named per
   row. Every row's evidence state stays WRITTEN-UNRUN until that pass.
3. Confirm §2's invariant-9 disposition independently: search
   `ledger/phase-08-advanced-coordination/` for P7.37 / the map-gap / the
   composition test; confirm nothing closes it.
4. Confirm the P9.28 linkage: the open rows in §4 are exactly the ones whose
   human runs are unscheduled.
5. Confirm no safety/certification/protection wording anywhere in this map.

**Filling commands (for the human gate):** after each pilot run named in a
row's "pilot live proving" cell, the runner updates THAT row's evidence
state with the run reference and date — per-row edits only, never a bulk
"all closed" claim; the §4 table is re-derived, not asserted.

---

## 7. Phase 7 follow-ups (carried; never closed by this map)

1. The invariant-9 end-to-end composition test — OPEN, owner the
   application/plugin integration owner (§2; also named for P9.27).
2. Platform-side §14.3 idempotency-key registry (`31_edge_rejection_proof.md`
   §7.1) — affects invariant 1/5 rows at the platform layer.
3. Cryptographic signature replacement for the `edge-sig:` scheme (P6.15
   deployment requirement; P9.09 owns the pilot key inventory).
4. Sequence-density decision (head gaps) — `31_edge_rejection_proof.md`
   §7.3; affects invariant 5's pilot configuration.
5. CSPRNG nonce derivation (P9.33) and SHA-256+ update verification (P9.34)
   — deployment requirements, unchanged.

## 8. Open risks to carry into P9.27

| # | Risk | Source |
|---|---|---|
| R-P9.35-1 | The invariant-9 composition map-gap: the plugin→feasibility-gate chain is proven per-layer, never end-to-end; Phase 8 did not close it | §2 of this record |
| R-P9.35-2 | The ledger-step-2 pilot tests (wrong-site identity, superseded context) have no scheduled run; P9.28 cannot approve a site while they are unrun | §3 invariant-1 row |
| R-P9.35-3 | Invariant 10's pilot evidence is entirely human drill output; no code test can supply it | §3 invariant-10 row |
| R-P9.35-4 | The pilot's cryptographic posture rests on integrity-checksum-grade primitives until the recorded replacements land | §5 |
| R-P9.35-5 | Head-gap sequence admission (monotonic, not dense) is the pilot's recorded design position until the Phase 7 follow-up decides otherwise | §3 invariant-5 row |
