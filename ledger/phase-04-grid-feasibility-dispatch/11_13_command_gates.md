# P4.11 + P4.12 + P4.13 — Decision note: the command gates (creation +
# validation, feasibility, authorization)

- **Date:** 2026-11-07.
- **Subtasks:** P4.11, P4.12, P4.13 (`ledger_phase_4.md`). Report trace:
  §14.1–§14.3, §12.5, §17.3–§17.5, §15.3, §21.4.
- **Deliverables:** `crates/flexcore-application/src/command_gates.rs` (NEW),
  `crates/flexcore-application/src/lib.rs` (additive decl + re-exports),
  `crates/flexcore-application/tests/command_gates.rs` (NEW),
  `crates/flexcore-authz/src/action.rs` (additive: `command.create`,
  `command.validate`, `command.approve`, `command.read`),
  `crates/flexcore-authz/src/phase4_policies.rs` (NEW),
  `crates/flexcore-authz/src/lib.rs` (additive decl),
  `crates/flexcore-authz/tests/phase4_policies.rs` (NEW), this note.
- **Reused, never restated:** the P4.10 lifecycle engine and its
  frozen-edge/gate/evidence rules (`flexcore_domain::command_lifecycle`);
  the P1.06 `DispatchCommand` §14.3 validators; the P3.13 staleness gates
  and the P3.22 revision-supersede discipline (via seams); the P4.03
  model-version health (Active-only rule, content hash); the P4.04–P4.07
  grid gate (via seam); the P1.08 transactional-outbox assembly; the
  P1.11 allow-only `PolicySet` with the P2.12 recorded-grants convention.
- **Not built here (recorded boundaries):** the §12.5 pipeline itself and
  the limit verdict are P4.06–P4.07's (concurrent); the reject-vs-HOLD
  policy table is P4.08's; the §14.3 field-level record schema and the
  payload rendering are P4.14's; the expiry sweep is P4.15's; ISSUED and
  everything past APPROVED is P4.16's.

## 1. Recorded decisions

| ID | Decision | Where |
|---|---|---|
| D-P4.11-1 | **Creation-source rule.** A command is created FROM AN APPROVED SCHEDULE only: the creation gate refuses a schedule revision that is SUPERSEDED or Unknown in the caller's tenant scope — rejected AT CREATION (named `CommandGateError::CreationRejected`, no record, no event), never merely at issue. The usability answer comes from the store seam, whose composition-root implementation MUST delegate to the P4.10 `CommandRecordStore::ensure_schedule_revision_usable` (which calls the P3.22 surface) — the two layers agree by construction. Unknown is never "usable". | `command_gates.rs` (`create_proposed_command`, `ScheduleRevisionUsability`); tests `creation_from_a_superseded_revision_is_rejected_at_creation_not_merely_at_issue`, `creation_from_an_unknown_revision_is_rejected_unknown_is_never_usable` |
| D-P4.11-2 | **Creation rejections are not lifecycle transitions.** A rejected creation never entered the lifecycle (the freeze's only entry is `[*] → PROPOSED`), so there is no REJECTED journal row and no `DispatchRejected` event — the rejection is the named typed error, audited by the caller. Gate rejections AFTER creation are lifecycle transitions (REJECTED with gate + failing check + evidence + `DispatchRejected`). | `command_gates.rs` (`CreationRejected`); the superseded-creation test asserts no record and no event |
| D-P4.11-3 | **Authorization-reference placeholder.** The §14.3 record shape requires an authorization decision reference, but at PROPOSED no authorization decision EXISTS. The creation gate records the placeholder `authz:pending:command/<id>` (the P1.06 constructor requires a non-empty reference); the REAL decision is the P4.13 `AuthorizationDecisionRecord` plus the journal's APPROVED entry — the evidence of record under the P4.10 content-immutability rule (D-P4.10-2). Replacing the placeholder in the stored record is P4.14's persistence concern. | `command_gates.rs` (`pending_authorization_reference`) |
| D-P4.11-4 | **Validation checks and order (§14.2 VALIDATED row).** Schema (re-checked through the existing `DispatchCommand`/`ControlTarget` validators), freshness (the P3.13 envelope gate on the source schedule's envelope; the referenced grid model must be ACTIVE per P4.03), asset checks (exists in the caller's tenant — a foreign and a missing asset are indistinguishable per P0.16 rule 5; capability model present; every targeted quantity declared — undeclared is never unconstrained), ramp (both required rates fit the declared capability), SOC (declaration; the [0,1] bound is structural in `StateOfCharge`), command policy (execution duration fits the validity window; optional deployment cap). The FIRST failing check names the rejection; every seam ERROR maps to a fail-closed check code, never a pass. | `command_gates.rs` (`validation_checks`); per-check tests |
| D-P4.11-5 | **Gate journaling.** Every gate result (pass or rejection) is journalled with the actor, the gate name, the evidence reference, and a controlled-clock timestamp; the evidence reference embeds the SAME pinned instant the transition carries. Timestamps come exclusively from the injected `LifecycleClock` — never wall-clock. | `command_gates.rs` (`PinnedClock`, all three gates); `every_gate_transition_journals_actor_and_controlled_clock_timestamp` |
| D-P4.12-1 | **Evidence separation is structural.** Validation evidence and feasibility evidence live on DISTINCT journal rows (→ VALIDATED vs → FEASIBLE, each naming its own gate) and on distinct result fields (`GateResult.evidence_reference` vs `FeasibilityGateResult.feasibility_evidence`); the P4.13 decision record cites both by their EXACT journal IDs. They are never merged, never reused across gates. | `command_gates.rs`; test `the_feasibility_gate_moves_validated_to_feasible_with_separate_evidence` |
| D-P4.12-2 | **Feasibility verdict seam.** flexcore-application cannot name flexcore-grid-api (the dependency rule; the manifest is not this subtask's file), so the §12.5 pipeline + P4.06/P4.07 verdict arrive via the `CommandFeasibilityRunner` seam; the composition root converts `run_feasibility_gate`'s `GatePipelineOutcome` plus the limit verdict into `FeasibilityGateVerdict` (`Passed` / `LimitViolated{constraint}` / `FailClosed{reason}`). The schedule ID + revision are passed FROM the command, so the refs cannot diverge from the command's binding. The seam pattern is the one recorded for the P4.04 gate sources. | `command_gates.rs` (seam statement, `CommandFeasibilityRunner`) |
| D-P4.12-3 | **Fail-closed rejection.** A `FailClosed` verdict (non-convergence, provider error, stale model) rejects the command through the feasibility gate with `fail_closed:<reason>` — §17.5 invariant 9. The reject-vs-HOLD classification is P4.08's policy table; when P4.08 lands, its HOLD policy maps onto the same verdict before the command gate (recorded boundary). | `command_gates.rs` (`FailClosed` branch); test `a_fail_closed_verdict_rejects_never_passes` |
| D-P4.12-4 | **Model-hash binding (the P4.16 hook).** Feasibility evidence references THE MODEL HASH in a recorded reference format (`feasibility:attempt/<id>:model/<mid>:hash/<hash>`, parsed by `parse_feasibility_evidence_hash`, plus the structured field on `FeasibilityEvidence`). `ensure_command_issueable` is the check P4.16's adapter calls before ISSUED: it requires state APPROVED, the feasibility evidence present in the journal, and a hash equal to the ACTIVE model version's hash; any mismatch/absence fails closed (`ModelEvidenceStale` / `MissingFeasibilityEvidence` / `InconsistentJournal`). | `command_gates.rs` (`ensure_command_issueable`); test `feasibility_evidence_binds_the_model_hash_and_a_model_change_blocks_issue` |
| D-P4.13-1 | **No auto-approve path exists.** APPROVED is produced by exactly ONE operation (`record_command_approval`), which requires the state FEASIBLE (the application-layer guard; the domain guard test is `flexcore-domain/tests/command_lifecycle.rs::approved_without_feasible_evidence_is_impossible_by_construction`), an ALLOW decision from the authorization evaluator at the recorded scope, scope coverage of the command's asset, separation of duties where configured, and a future expiry. There is no auto-approve action on the authz surface either (`command.auto_approve` parses to nothing). | `command_gates.rs`; `approval_is_impossible_without_feasible_the_guard_at_both_layers`; authz tests |
| D-P4.13-2 | **Authorization decision record.** The recorded facts of one approval: decision reference (`authz-decision:command/<id>@<t>`, the journal's evidence reference and the THIRD distinct evidence surface), authorizing actor, role, approved scope (recorded text `tenant` / `site:<id>` / `asset:<id>`), approved limits, decision timestamp, expiry (§17.3), the separation-of-duties policy APPLIED, and the exact validation + feasibility evidence IDs covered (taken from the journal, never caller input). `DispatchApproved` is emitted on approval; `DispatchRejected` on denial. | `command_gates.rs` (`AuthorizationDecisionRecord`); test `approval_by_a_distinct_authorized_actor_records_the_full_decision` |
| D-P4.13-3 | **Separation of duties.** Enforced by default (`CommandGatePolicy::default`): the approver identity must differ from the command's proposing identity (`intent.actor_or_service`, §14.3 field 5). A same-identity approval is a DENIAL (FEASIBLE → REJECTED, reason + denying actor). `NotConfigured` is a recorded deployment setting — the decision record then says `not_configured` explicitly, never silently. The policy LAYER contributes the role split (no Operator approval grant in `phase4_policies`), the gate contributes the identity split. | `command_gates.rs`; `self_approval_is_rejected_where_separation_of_duties_is_enforced`; `separation_of_duties_not_configured_is_recorded_not_silent` |
| D-P4.13-4 | **Recorded Phase 4 grants (RECOMMENDED DEFAULTS, reviewable).** `command.create`/`command.validate`: Operator + TenantAdmin at tenant/site/asset kinds. `command.approve`: TenantAdmin ONLY, at site and asset kinds (resource/site/asset-scoped approval per §17.3; NO tenant-level approval grant; PlatformAdmin gets no bypass). `command.read`: Operator, TenantAdmin, Viewer. Everything else denies by default; `dispatch.authorize`/`dispatch.issue` stay OFF the action surface (§5.7). The action-surface exclusion note in `action.rs` was updated to record that the `command.*` actions are the human approval surface, not plugin capabilities. | `phase4_policies.rs`; `action.rs`; `tests/phase4_policies.rs` |
| D-P4.13-5 | **Denials are gate transitions, not errors.** An authorization denial (role, scope coverage, separation of duties, expired authorization) moves FEASIBLE → REJECTED with the denial reason and the denying actor (§14.1 edge 7; ledger P4.13 step 3). An already-expired authorization (expiry ≤ controlled-clock now, end-exclusive) cannot approve; later APPROVED → EXPIRED handling is P4.15's. | `command_gates.rs` (`reject_authorization`); expiry + denial tests |

## 2. Gaps and limitations (recorded, none silently filled)

- **No `DispatchValidated` / `DispatchFeasible` events:** the frozen §15.3
  vocabulary has none and none may be invented (P0.14). The VALIDATED and
  FEASIBLE facts live in the transition journal (the evidence of record);
  `DispatchProposed`, `DispatchRejected`, and `DispatchApproved` are
  emitted.
- **Command payload rendering is caller-supplied:** the store row's
  `command_payload_json` is the composition root's serialization seam
  (flexcore-application has no serializer); the gate checks it non-empty
  but cannot verify it matches the built `DispatchCommand`. The full §14.3
  field-level record schema is P4.14's.
- **The `phase2_default_policies` module named in the task does not exist**
  in flexcore-authz (the crate holds `action`/`decision`/`policy`/`scope`
  only). The P2.12 recorded-grants CONVENTION (stable rule references as
  allow evidence, explicit grants, deny-by-default) is what the new
  `phase4_policies` module follows; no phase2 function was modified
  (there is none to modify).
- **Command content vs stored state (TOCTOU):** gate inputs carry the
  `DispatchCommand` (content is immutable once inserted, D-P4.10-2); the
  store is authoritative for STATE (compare-and-swap on every journal
  write). The replay re-walks the persisted journal through the domain
  engine so every gate transition applies from the true current state; an
  unmappable journal entry fails closed.
- **Research cut-off:** no external sources were consulted; nothing in
  this batch changes the 7 November 2026 cut-off.

## 3. Review status and verification

- **Security review sign-off: PENDING** (the standing status for this
  phase). A security engineer must confirm D-P4.13-1 (no auto-approve),
  D-P4.13-3 (SoD default enforced), D-P4.13-4 (the grant matrix), and
  D-P4.13-5 (denials as transitions) before the ledger P4.13 checkbox is
  ticked. Power-systems review of the validation-check set (D-P4.11-4)
  is likewise pending.
- **Not run (VERIFY-BUILD):** per the task constraint, `cargo build`,
  `cargo test`, `cargo clippy`, and `cargo fmt` were NOT executed for
  these files. Everything is reviewed-by-construction against the P4.10
  domain engine, the P1.08 outbox assembly, and the P1.11 policy types.
- **VERIFY-BUILD (outbox subject):** `emit` renders the destination
  subject via `flexcore_events::subject::subject_for`; this batch assumes
  the frozen scheme handles the three dispatch events — the first
  `cargo test -p flexcore-application` run must confirm.
- **VERIFY-BUILD (seam implementations):** the six port traits here have
  no production implementation yet; the composition root must implement
  them over `flexcore_storage::command_store::CommandRecordStore`, the
  P3.13 staleness gates, the P4.03 registry, the Phase 2 registry +
  P3.04 capability data, `flexcore_grid_api::feasibility::
  run_feasibility_gate` + the P4.06/P4.07 verdict, and
  `flexcore_authz::policy::PolicySet` + `phase4_command_policies`. The
  `CommandGateStoreError` vocabulary maps 1:1 from `CommandStoreError`.
- **No forbidden edits:** flexcore-domain, flexcore-storage,
  flexcore-grid-api, flexcore-events, the adapters, the manifests,
  `tools/check_dependency_rule.py`, and the ledger checkboxes are
  untouched.

## 4. Control status

Validation, feasibility, and authorization are distinct gates (§14.1);
network feasibility is not operator authorization. The command lifecycle
is a protected platform invariant owned by the platform kernel (§0.2,
§5.7): plugins may propose and request validation but can never authorize,
issue, or select the control-authority mode — nothing in the recorded
grants gives a plugin any `command.*` action. The entire 0.x line is
simulation-only (§21.4): an APPROVED command authorizes a simulated issue
(P4.16) at most, never a device write; ISSUED is not recorded here.
FlexCore performs no primary frequency response, synthetic inertia, relay
protection, inverter inner-loop control, or sub-cycle control (§14.5).
