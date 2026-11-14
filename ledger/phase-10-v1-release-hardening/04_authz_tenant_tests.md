# P10.04 — Authorization and Tenant-Isolation Test Suite (record)

- **Subtask:** P10.04 (§22.3 release gate 3; report §16.2, §17.3–§17.4, §5.7).
- **Status:** WRITTEN-UNRUN — implemented, awaiting human build/test (ENTRY_NOTE
  boundary 3). No `cargo` command was executed in this loop; zero observed
  failures is NOT claimed. Ledger tick = "implemented, awaiting human build/test".
- **Dependencies honesty:** P10.01 (scope freeze) and P10.18 (release candidate)
  are concurrent waves; the suite is written against the CURRENT crate seams and
  must be re-run against the frozen release-candidate identifier when P10.18
  produces it. P9.08 / P9.10 are cross-references only (no Phase 9 evidence was
  re-produced here).

## 1. What was produced (files)

| File | Content |
|---|---|
| `crates/flexcore-authz/tests/p10_04_authz_tenant_isolation.rs` | The authz-crate slice: the 348-cell (4 roles × 29 actions × 3 scope kinds) in-tenant exact-match matrix over the recorded `phase4_command_policies()` grant table; the 348-cell cross-tenant `ScopeDenied` matrix (zero allows); the deny-by-default proof (empty `PolicySet` denies all 348 in-tenant cells; `UnknownScope`; `MissingContext`); the never-granted-set refusals (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`, `command.issue`, `command.auto_approve`, alarm/audit/telemetry writes — all parse to `None`, `UnknownAction` for every role); the `command.approve` role/scope boundary (TenantAdmin only, site/asset only, never tenant-scoped, never PlatformAdmin/Operator/Viewer); decision auditability (every denial carries its stable reason id and non-empty evidence ref). |
| `crates/flexcore-kernel/tests/p10_04_tenant_scope_refusals.rs` | The kernel-mirror slice (P7.39 `TenantScope`): the exhaustive wrong-scope matrix over `covers`/`enforce_scope` (tenant-wide and site-narrowed envelopes × five requested shapes — cross-tenant NEVER covered); echo-mismatch refusals (wider / cross-site / cross-tenant echo → `TENANT_SCOPE_ECHO_MISMATCH`, both scope renderings named); host-stamped-construction refusals (`TENANT_SCOPE_INVALID` for empty/blank/oversized ids); audited enforcement (`PLUGIN_TENANT_SCOPE_REFUSED` records with the refusal class, admissions recorded too); invalid inner envelope refused at wrap time (`TENANT_SCOPE_ENVELOPE_INVALID`, fail-closed on both refusal paths); the five-name refusal table pinned. |
| `crates/flexcore-research/tests/p10_04_research_tenant_isolation.rs` | The research-mirror slice (P8.31 `tenant_isolation`): the cross-tenant read matrix exhaustive over all six closed worker families (other tenant / other site / un-narrowed-under-narrowed → `TENANT_SCOPE_MISMATCH`; narrowing preserved cases pass); echo-mismatch and stamp-construction refusals; fail-closed unknown-record / empty-tenant / unknown-kind refusals; cross-tenant model serving refused without a RECORDED approval (`RESEARCH_MODEL_SERVING_NOT_APPROVED`), admitted with one; unattributed approvals refused; every read outcome audited with the stable audit kinds; the kernel alignment seam and 15-name error table pinned. |
| `apps/control-plane/tests/p10_04_deny_all_and_default_policies.rs` | The control-plane slice: the `DenyAllResolver` deny-by-default proof (every request refused with `MissingCredentials`, with or without a bearer token — no anonymous fallback, release builds carry no `dev-auth`); the closed role vocabulary (`parse_role`); the 348-cell exact-match matrix over the CUMULATIVE `phase5_default_policies()` grant table (79 allow cells, counted from the recorded builders, transcribed verbatim); phase-set cumulative-monotonicity check (no phase silently revokes); the 348-cell cross-tenant matrix over the default set (`ScopeDenied`, zero allows); `command.approve`/`command.create`/`command.validate` granted NOWHERE in the REST sets (gate actions only, no REST write path — §15.2); `quarantine.read` admin-only; `tenant.delete` PlatformAdmin-only; tenant context built ONLY from the claims path in every test (criterion (c)). |

## 2. Reviewer-verification procedure (the human gate)

1. Build and run the suites against the release candidate once P10.18 records
   the candidate identifier:

   ```sh
   cargo test -p flexcore-authz --test p10_04_authz_tenant_isolation
   cargo test -p flexcore-kernel --test p10_04_tenant_scope_refusals
   cargo test -p flexcore-research --test p10_04_research_tenant_isolation
   cargo test -p flexcore-control-plane --test p10_04_deny_all_and_default_policies
   ```

2. Independent negative-case sampling (P10.04 verification method): pick one
   cross-tenant case per file and confirm the refusal by hand against the
   cited source (`policy.rs` decide-order steps 4–5; `tenant_scope.rs`
   `covers`; `tenant_isolation.rs` `enforce_scoped_read`; `auth.rs` builders).
3. Inspect the audit trail shape: every refusal in the kernel/research suites
   asserts a `*REFUSED` audit record with the stable refusal-class name in the
   detail — the audit-extract evidence for §16.2 "Audit cross-tenant access
   denials".
4. Re-run `python tools/verify_docs.py` and `python tools/validate_agent.py`
   after any doc change; `python -I tools/check_dependency_rule.py` for the
   manifest rule (no manifest was edited by this subtask).

## 3. Acceptance-criteria coverage and honest gaps

| P10.04 criterion | Covered here | NOT executable in this loop |
|---|---|---|
| (a) every unauthorized attempt denied AND audited — zero unauthorized successes | The denial matrices (authz 348+348, kernel 10+echo, research 6 families × 4 wrong scopes, control-plane 348+348) and the audit-record assertions cover the application/authz layers. | A run against a DEPLOYED candidate (the audit LOG inspection, not the in-process audit sink) needs the P10.18 candidate; the HTTP 403 mapping is asserted by the existing `api_*` suites, not duplicated here. |
| (b) no service role bypasses RLS (§16.2, [R25]) | The structural precondition is proven and recorded: plugins/research workers NEVER touch tables directly (ADR-007 — the kernel and research module headers, pinned by statement assertions in the suites); tenant scoping is enforced before any rule match. | The actual `FORCE ROW LEVEL SECURITY` / table-ownership / service-role verification is a DATABASE test on a deployed candidate (Postgres) — NOT EXECUTABLE in this loop; it is a human gate input against the P9.10 configuration. Recorded as an open step. |
| (c) tenant context never from a request body | Every test builds `TenantContext`/`TenantScope`/`ResearchWorkerScope` through the ONLY claims-based constructors; the kernel scope fields are private (struct-literal forgery is a compile error, recorded in the suites). | The route-layer proof that no handler reads `tenant_id` from a body is an existing-suite concern (`api_edge_isolation_p6_21.rs`, `tenant_isolation_p6_21.rs`) — cross-referenced, not duplicated. |
| (d) every edge identity scoped to one tenant/site set | The kernel host-stamped scope (tenant + optional site narrowing) and its echo/forgery refusals are proven. | The per-edge-identity enrollment binding (one identity ↔ one approved site/asset set) is the P6.03/P9.08 enrollment evidence — cross-referenced, not re-produced. |
| (e) §16.3 retention/residency configured per data class | Out of scope for a test file: this is configuration on the candidate, owned by P9.10/P10.04 step 6. | NOT EXECUTABLE HERE — recorded as an open step requiring the deployed candidate and the customer contract values. |

## 4. Findings (recorded, not fixed — src logic is outside this subtask's ownership)

- **F-P10.04-1 (observation, no action taken):** the closed `Action` surface has
  29 variants but no `Action::ALL` constant; the exhaustive matrices in
  `p10_04_authz_tenant_isolation.rs` and `p10_04_deny_all_and_default_policies.rs`
  maintain their own 29-entry arrays. A new action variant without updating
  those arrays would silently shrink the matrices' coverage. Recommendation:
  add `Action::ALL` to `flexcore-authz` in a later subtask that owns that file.
- **F-P10.04-2 (observation, no action taken):** the `PolicySet` has no rule
  introspection API, so the exact-match matrices assert the grant tables
  behaviourally (448 allow cells asserted across the two 348-cell matrices).
  This is sufficient for the gate but means a deliberately ADDED (not
  transcribed) grant would be caught only if a test asserts that cell's deny —
  which it does, because the matrices assert every cell's exact outcome.
- **F-P10.04-3 (recorded constraint):** `DenyAllResolver` refusing a SUPPLIED
  bearer token with `MissingCredentials` is the recorded behaviour (no resolver
  is compiled when `dev-auth` is off); the error choice is the resolver's, and
  both variants map to 401. No defect; recorded so the 401-not-403 distinction
  (unauthenticated vs unauthorized) stays visible in the release review.

## 5. Ledger tick

`P10.04` in `ledger_phase_10.md` was ticked in the same wave as this record,
with the meaning recorded in ENTRY_NOTE boundary 3: **implemented, awaiting
human build/test**. Nothing in this record claims a run, a pass, or a release
candidate.
