# flexcore-authz (P1.11)

Authorization decision types, scope types, and deny-by-default policy
evaluation for the Phase 1 API surface (report §8.1 `flexcore-authz`,
§17.3 control 2 "RBAC plus resource/site/asset-scoped authorization",
§16.2; ledger P1.11; P0.16 tenancy rules, P0.17 threat-model freeze).

## The deny-by-default rule

**Everything that is not explicitly granted is denied, with a reason.**

- The `PolicySet` starts empty and denies everything; capability arrives only
  through explicit `grant(role, action, scope_kind, rule_reference)` calls.
- An action id outside the known Phase 1 surface → `Deny(UnknownAction)`.
- A missing scope → `Deny(UnknownScope)` (there is **no default scope**
  anywhere in this crate; `PolicySet::evaluate` takes the scope as an
  explicit parameter).
- A missing tenant context → `Deny(MissingContext)`.
- A scope whose tenant differs from the context tenant → `Deny(ScopeDenied)`
  (audited per P0.16 rule 6).
- A request matching no allow rule → `Deny(NoMatchingRule)`.
- Scope levels do not inherit: a tenant-level grant does not authorize a
  site- or asset-level request, and no role — including `PlatformAdmin` —
  bypasses the tenant match. Breadth comes from more rules, never from a
  bypass.

## The decision-carries-reason rule

**Decisions are explicit values, never booleans without a reason** (ledger
P1.11 step 2).

- `PolicySet::evaluate` returns `AuthorizationDecision` — `Allow` or `Deny` —
  and every value carries an `AllowReason`/`DenyReason` plus an
  `EvidenceRef` (for allows: the matched rule's reference; for denies: a
  canonical deny reference such as `authz:deny:scope-denied`).
- There is **no `From<AuthorizationDecision> for bool`**, no `Into<bool>`,
  and no `evaluate -> bool` anywhere in the crate. `is_allowed()` exists for
  control flow only: callers must log/audit the full decision — reason and
  evidence — before acting on it (the P1.10 `AuditSink` is the evidence
  sink; §17.3 control 12; P0.16 rule 6).

## Boundary statement — this crate is NOT the dispatch authorizer

This crate holds authorization **policy types and their evaluation** for the
Phase 1 API surface (portfolio/site/asset registry actions). The protected
kernel — **final dispatch authorization** and dispatch state transitions — is
a **later phase (Phase 4)**: `dispatch.authorize` is **never granted to
general plugins** and `dispatch.issue` is owned by the protected dispatch
service and protocol adapter under a signed authorization token (report
§5.7 permission table; P0.17 threat-model freeze; P0.19 plugin model).
Nothing in this crate may be presented as the dispatch authorizer, and the
evaluation here is protected platform behavior, not a plugin-replaceable or
plugin-configurable seam (P0.17; §5.7: a plugin may propose work, it must not
grant itself authority).

## Tenant context

Evaluation runs against the **domain** `TenantContext` (P1.06,
`flexcore_domain::tenant_context`), whose only constructor is
`TenantContext::from_authenticated_claims`. That constructor is the single
place claims are validated, and `AuthenticatedClaims` has no public
constructor anywhere: building a context from a raw string, a bare
`TenantId`, or a request field is a **compile error**, not a runtime check.
This crate consumes the context read-only (`tenant_id()`) and adds no
construction path.

## What is deliberately not modeled in Phase 1

- **Deny rules / deny-beats-allow priority.** The `PolicySet` is allow-only;
  denial is structural (deny-by-default). `DenyReason::ExplicitDeny` is
  reserved vocabulary for the later policy work and is never produced by the
  Phase 1 evaluator.
- **Role hierarchy.** Roles are flat; `Viewer` gets nothing from an
  `Operator` grant and vice versa. Every (role, action, scope-kind) triple
  needs its own explicit grant.
- **Ownership-based implicit grants.** Owning a site does not implicitly
  authorize actions on it; ownership lives in domain state (P1.06 entities),
  not in this evaluator.
- **Dispatch, command, or plugin-capability actions.** The `Action` surface
  is the Phase 1 registry surface only; dispatch authorization is the Phase 4
  protected kernel (see the boundary statement).

## Verification

Deny-by-default is proven per case in the unit tests (ledger P1.11
acceptance criteria): unknown action, unknown scope, missing context,
cross-tenant scope mismatch, no-matching-rule on an empty policy set, and the
allow path with the matched rule reference as evidence. The tenant-context
bypass attempts are compile-level facts, documented in the `policy` test
module the same way the domain crate documents them (the compiler is the
oracle; the positive claims→context path is proven in
`flexcore-domain/src/tenant_context.rs` tests). Build and tests are run by
the human (`cargo test -p flexcore-authz`); the dependency rule is checked
with `python tools/check_dependency_rule.py`.
