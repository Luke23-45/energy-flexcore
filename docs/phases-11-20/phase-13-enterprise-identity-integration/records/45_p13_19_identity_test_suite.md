# P13.19 — Identity test suite (WRITTEN-UNRUN: authz matrix extension, SCIM conformance scenarios, step-up negatives, webhook signature/tolerance/replay, idempotent-handler bookkeeping)

| Field | Value |
|---|---|
| Subtask | P13.19 — identity test suite (written-unrun): authz matrix extension tests (new scoped actions/roles incl. SiteApprover + `apikey.*`), SCIM conformance scenarios, step-up negative tests (expired/replayed/failed challenges — extending record 04's matrix), webhook signature/tolerance/replay tests (extending record 21's surfaces), idempotent-handler tests for delivery bookkeeping |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | WRITTEN-UNRUN: every test file below is written complete and NEVER compiled, executed, or passed. **Execution is the human build/test gate (conventions §1 item 3)** — no result cell exists anywhere in this record and none may be invented. The first workspace build is the gate that even compiles these files. |
| Sources | conventions §1 item 3 (the write-don't-run rule); record 05 §6/§8 (the in-crate authz core this extends); record 04 §7/§9 (the in-crate step-up matrix this extends — consumed, never redefined); record 21 §1–§3 (the webhook contract surfaces this extends); record 03 §5/§7 (the SCIM conformance posture — NO conformance claim, the scenarios are written-unrun and lab-gated on P13.20); record 42 §8 (the session slice); the existing test-file conventions (`crates/flexcore-authz/tests/security_authz.rs`, `apps/control-plane/tests/api.rs`, the console vitest files) |
| Files touched | `crates/flexcore-authz/tests/phase13_authz_matrix_tests.rs` (NEW), `apps/control-plane/tests/phase13_stepup_negative_tests.rs` (NEW), `apps/control-plane/tests/phase13_session_resolver_tests.rs` (NEW), `apps/control-plane/tests/phase13_webhook_delivery_tests.rs` (NEW), `apps/control-plane/tests/phase13_scim_role_mapping_tests.rs` (NEW); console tests were delivered with their owning subtasks (records 43/44: `console/src/auth/*.test.ts`, `console/src/components/sessionBanner.test.tsx`, `console/src/workspaces/adminManagement*.test.*`) |

## 1. The test inventory (all WRITTEN-UNRUN)

| File | Extends | Scenarios (count) | Honest state |
|---|---|---|---|
| `crates/flexcore-authz/tests/phase13_authz_matrix_tests.rs` | record 05 §3 (the grant table), the P1.23 security suite | SiteApprover allows at site/asset scope with phase-13 evidence refs; denies at tenant scope; denies cross-tenant; the 15-action administrative sweep denies; `apikey.*` grants only the two admin roles at tenant scope; denies Operator/Viewer/SiteApprover; denies at site/asset kinds; denies cross-tenant; the never-granted names do not parse AND deny through `evaluate_action_id` for all five roles; the role-id vocabulary is one distinct set (11 tests) | Written-unrun; pure policy evaluation, no grants made |
| `apps/control-plane/tests/phase13_stepup_negative_tests.rs` | record 04's matrix (§7 in-crate core) | EXPIRED: denial at the exclusive window close; success 1 ms before; evidence never outlives the window. REPLAYED: after-success replay denies; clock-rollback replay denies. FAILED: verifier rejection is a named denial; the failed attempt consumed the challenge (a fresh challenge is the retry path). BINDING: command-A evidence never verifies command-B; console-action vs command-approval never match; subject/tenant mismatch deny before the verifier runs. MISSING: unknown id; empty subject never issues; `DenyAllStepUpVerifier` denies everything fail-closed; the no-wedge chain (failures never wedge a fresh challenge) (11 tests) | Written-unrun; local test doubles only — no IdP, no MFA factor, no challenge ever issued in any environment |
| `apps/control-plane/tests/phase13_session_resolver_tests.rs` | record 42 §1/§7 (the resolver-layer semantics) | The registered defaults re-asserted; revoked-denies-at-the-resolver-with-valid-token; the bare-trait fail-closed shape; idle timeout + no-extension-on-deny; absolute lifetime over activity; binding mismatch; missing/blank/unknown references; tenant-wide revocation scoped exactly; subject-scoped revocation (the record 03 §4 deprovisioning seam); the cap refuses the newest login with independent counters; the list shows only live sessions (11 tests) | Written-unrun; in-memory directory fixture only |
| `apps/control-plane/tests/phase13_webhook_delivery_tests.rs` | record 21 §1–§3 (the delivery contract) | Sign/verify round-trip over the RAW body; a re-serialized body never verifies; tolerance inside-accepts / at-boundary-rejects / both sides absolute (future-stamped equally rejected); the replay model stated honestly (exact replay inside the window still verifies — the window + receiver idempotency are the defense; outside rejects); tampered timestamp/body break the signature; rolling overlap verifies both, closure revokes the previous; idempotency key collapses duplicate enqueues; same-key-different-content is a named conflict; the key is per-tenant; failed attempt schedules a retry (not due before backoff); exhaustion dead-letters with depth 1; the registered backoff schedule re-asserted (12 tests) | Written-unrun; in-memory store fake + local HTTP double; NO webhook ever signed or delivered |
| `apps/control-plane/tests/phase13_scim_role_mapping_tests.rs` | record 03 §3 (the single vocabulary) | Six named scenarios: group-named-for-role maps exactly; login and provisioning map ONE set; unmapped groups contribute nothing / empty → reject (never defaults); first-mappable-group wins (no union); the never-granted names are never group ids or actions; the vocabulary cannot fork between paths (paired with the authz file's list pin) (6 tests) | Written-unrun; the FULL protocol scenarios (POST/PATCH/DELETE `/Users`, the deprovisioning window) are the UNRUN tables in §3 — no SCIM service exists to run them |

## 2. What these files deliberately do NOT do

| Not done | Why |
|---|---|
| No test is run, compiled, or "passed" | conventions §1 item 3 — the human gate; this record contains no result cell and quotes none |
| No new test dependency added | the hard ban; every file uses the existing workspace dev surfaces (`flexcore-authz`, `flexcore-control-plane`, `flexcore-domain`, `flexcore-contracts`, std) |
| No SCIM protocol test against a live or fake server | no SCIM service exists (record 03 §5); the protocol scenarios are recorded, not coded — coding them would fabricate a server that does not exist |
| No claim of crypto correctness | the RFC 4231 / FIPS vector tests live in the signing module's own tests (record 21 §4); this suite does not duplicate or re-verify them |
| No console test duplicated here | the console tests landed with records 43/44; this record cites them, never restates them |

## 3. The SCIM protocol scenarios (recorded UNRUN — lab-gated on P13.20)

The full RFC 7644-shaped scenarios the code slice cannot yet carry. Each
is a scenario TABLE — written before any run that could fill its result
cell (X-8); every Result cell is EMPTY.

| # | Scenario (per record 03 §2) | Precondition | Expected enforcement | Result |
|---|---|---|---|---|
| SC-1 | `POST /Users` (create) via the SCIM connection | the P13.20 lab IdP pushes a user into tenant T | the tenant-scoped record carries SCIM id/userName/active/groups; NO role granted at creation | EMPTY (UNRUN) |
| SC-2 | login of the created user with no mapped group | SC-1 | the resolver REJECTS (no role, never defaults to Viewer) | EMPTY (UNRUN) |
| SC-3 | `PATCH /Users/{id}` adding a mapped group | SC-1 + the mapping table | the mapping inputs update; the next login resolves the mapped role | EMPTY (UNRUN) |
| SC-4 | `PATCH` groups change (role change) | SC-3 | the next login resolves the NEW role; the old role's grants no longer apply | EMPTY (UNRUN) |
| SC-5 | `active=false` (deactivate) | SC-3 | within the recorded 5-minute window (REGISTERED RECOMMENDATION), the resolver denies the user regardless of token validity; the enforcement instant is audit-recorded | EMPTY (UNRUN) |
| SC-6 | `DELETE /Users/{id}` | SC-3 | same enforcement as SC-5 | EMPTY (UNRUN) |
| SC-7 | a group change pushing the user into NO mapped group | SC-3 | the next login rejects | EMPTY (UNRUN) |
| SC-8 | a delivery failure on the SCIM connection | any | the failure is recorded (never silent); a stale connection past the registered threshold is flagged | EMPTY (UNRUN) |

## 4. The frozen sets — byte-identical (verified by construction of this batch)

| Frozen set | How this batch touches it | State |
|---|---|---|
| The never-granted permission set (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) | the authz and SCIM test files ASSERT its unreachability; no source file of the enum or policies was opened for edit | byte-identical |
| The frozen 14-state / 27-edge command lifecycle | no test references command-state internals; `crates/flexcore-domain/src/command_state.rs` untouched | byte-identical |
| The frozen 26-event vocabulary | the webhook tests use event SUBJECT strings only; `crates/flexcore-events/src/events.rs` untouched | byte-identical |

## 5. ASSUMPTION / UNRUN register

1. **UNRUN (all)** — every test in every file; no result exists; the
   first human build compiles them and the first `cargo test` runs them;
   any failure is a defect to fix, never a result to record here.
2. **ASSUMPTION (compile)** — the files compile against the current
   public APIs they consume (verified by reading the sources, not by
   building): `AuthorizationDecision::Allow(_, EvidenceRef)`,
   `evaluate_action_id`, `StepUpService`/`SessionEnforcingResolver`
   constructors, the webhook dispatcher/dispatcher ports,
   `map_role_claims`/`parse_role`. A signature drift surfaces at the
   first build.
3. **RECORDED LIMITATION** — the step-up/session tests use local fixture
   doubles for the verifier and inner resolver; they assert the protocol
   and gate semantics, never IdP behavior (the IdP is P13.20's lab gate).
4. **RECORDED LIMITATION** — the webhook replay tests state the honest
   model: an exact replay inside the tolerance window is
   indistinguishable from a legitimate duplicate at the signature layer;
   the window + receiver idempotency are the defense (plan §2.3's
   "handlers must be idempotent" — the sender-side collapse tests pin the
   bookkeeping half).

## 6. Handoff notes

1. **Human gates**: (a) the first workspace build (compiles all five
   files); (b) `cargo test --workspace` (runs them; failures are
   defects, not evidence); (c) the console install (`npm test` over the
   records 43/44 test files); (d) the P13.20 lab run fills §3's result
   cells.
2. **P13.22 (close)**: the test files extend exactly the records named
   in §1's "Extends" column — the consistency audit checks those
   cross-references both ways.
3. **Future waves**: extend these files additively; do not fork per-phase
   copies (the P1.23 suite is the precedent this batch followed).
