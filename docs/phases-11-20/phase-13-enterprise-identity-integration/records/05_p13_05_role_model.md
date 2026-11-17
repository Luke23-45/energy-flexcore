# P13.05 — Role model refinement (site-scoped approval duty; the LR-049 direction; ABAC-lite on the typed scopes)

| Field | Value |
|---|---|
| Subtask | P13.05 — portfolio/site-scoped permissions (ABAC-lite) layered on the typed scope vocabulary; new scoped actions extend `Action` ADDITIVELY; finer-grained approver roles separate administrative rights from approval duty; the never-granted permission set stays byte-identical |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | WRITTEN-UNRUN: the additive vocabulary extensions are implemented code (never compiled — the first human build is the gate); no role is assigned, no grant is evaluated in production, no deployment consumed any of it. Every operational evidence cell is EMPTY/UNRUN. |
| Sources | LR-049 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8: "Approver shares TenantAdmin with administrative rights; finer role needs a reviewed change"); `crates/flexcore-authz/src/scope.rs` (the typed scope machinery); `crates/flexcore-authz/src/phase4_policies.rs` (the recorded Phase 4 grant set — consumed, never edited); conventions §6.4 |
| Files touched | `crates/flexcore-authz/src/policy.rs` (additive: `Role::SiteApprover` variant + `id()` arm), `crates/flexcore-authz/src/action.rs` (additive: the four `apikey.*` variants + id/parse arms + negative-test additions), `crates/flexcore-authz/src/phase13_policies.rs` (NEW), `crates/flexcore-authz/src/lib.rs` (additive module line, marked), `apps/control-plane/src/auth.rs` (additive: the `site-approver` arm of `parse_role`) |

## 1. What LR-049 said and what this changes

LR-049 records that the approving authority is forced to be a
TenantAdmin — a role that also carries every administrative right — and
that a finer role needs a reviewed change. This subtask IS that reviewed
change (written-unrun): a dedicated approval-duty role whose ONLY grant
is `command.approve` at site/asset scope. The separation is now
expressible: "who may approve" is no longer forced to equal "who
administers the tenant".

## 2. The additive vocabulary extensions (the closed-enum rule, applied)

| Extension | Where | Additive shape |
|---|---|---|
| `Role::SiteApprover` (`site-approver`) | `policy.rs` | a variant, its stable `id()` arm, and explicit grants in the NEW `phase13_policies` module — no existing grant touched |
| `Action::ApiKeyCreate/Rotate/Revoke/Read` (`apikey.create` / `apikey.rotate` / `apikey.revoke` / `apikey.read`) | `action.rs` | variants + `id()`/`parse()` arms + explicit grants at TENANT scope only; the P13.08 record is the design home |

Nothing was removed, renamed, renumbered, or re-ordered; every earlier
grant set (`phase1_default_policies` … `phase5_default_policies`,
`phase4_command_policies`) is byte-identical.

## 3. The recorded Phase 13 grant set (`phase13_policies`)

| Action | Roles | Scope kinds | Rationale |
|---|---|---|---|
| `command.approve` | SiteApprover | site, asset | The LR-049 separation: approval duty at resource scope, zero administrative rights. NO tenant-level grant (the Phase 4 rule stands; this module widens nothing) |
| `apikey.create` / `apikey.rotate` / `apikey.revoke` / `apikey.read` | PlatformAdmin, TenantAdmin | tenant | API keys are tenant-scoped credentials administered by tenant/platform administrators (P13.08) |

Deliberate absences (asserted by tests): SiteApprover has NO grant for
any registry write, delete, API-key, or command create/validate action;
Operator/Viewer/SiteApprover have NO `apikey.*` grant; no tenant-level
`command.approve` grant exists for anyone; the four never-granted names
are structurally unreachable (§5).

## 4. The ABAC-lite statement (what "portfolio/site-scoped permissions" means here — and what it does not)

| IS (the existing machinery, now used deliberately) | IS NOT (deliberately) |
|---|---|
| A grant is a (role, action, scope-kind) triple; evaluation matches the scope KIND exactly (scope levels do not inherit — `scope.rs`); the scope carries its tenant and the cross-tenant match denies | No new scope kind (portfolio is expressed through the tenant/site/asset levels as P1/P2/P4 recorded them) |
| A site-scoped approver covers exactly the sites/assets their scope names — `ApprovalScope::covers` (Tenant ⊇ site ⊇ asset) is the coverage rule the P4.13 gate already enforces | No rule-expression language, no attribute predicates beyond the typed scope, no runtime policy editing |
| The `phase4_policies` module header already recorded site/asset-kind grants for `command.create`/`validate` — Phase 13 adds the matching approval-side role | No per-portfolio scope kind was invented (the portfolio is not a scope level in the typed vocabulary; asserting it would restate — not extend — the P1.11 freeze) |

## 5. The never-granted permission set — byte-identical, verified

Quoted verbatim, unchanged (conventions §6.4):

> The never-granted permission set (`dispatch.authorize`,
> `dispatch.issue`, `secret.read`, `network.any`) stays byte-identical

Verification: the `action.rs` negative tests assert all four names still
parse to `None` (deny `UnknownAction`), so no principal — human, service
account, API key, or machine identity — can request or be granted any of
them; the `phase13_policies` tests re-assert the same. Nothing in this
subtask touches any dispatch path; adapters reach dispatch only through
the command lifecycle.

## 6. Tests (written, never run)

`phase13_policies.rs` `#[cfg(test)]`: SiteApprover approves at site and
asset scope and denies at tenant scope; the administrative-surface sweep
(15 actions) denies for SiteApprover; `apikey.*` grants only to
PlatformAdmin/TenantAdmin at tenant scope and denies for
Operator/Viewer/SiteApprover; the four never-granted names stay
unparseable. `action.rs` tests: the new ids round-trip; the apikey
near-miss negatives (`apikey.secret`, `apikey.approve`, `apikey.issue`)
deny. All UNRUN — the human build/test gate executes them.

## 7. ASSUMPTION / UNRUN register

1. **UNRUN** — nothing compiled, no grant evaluated, no role assigned.
   Evidence cells EMPTY.
2. **RECORDED DEFAULT, reviewable** — the grant table in §3 is the
   recommended default (the `phase4_policies` convention); a deployment
   needing stricter shapes builds its own `PolicySet` by not using this
   builder — there is no override flag and no wildcard.
3. **RECORDED LIMITATION** — adding `Role::SiteApprover` requires every
   consumer that exhaustively matches `Role` to gain an arm; the
   grep-level audit found `Role::id()` as the only exhaustive match in
   library code (`apps/sim-runner`'s `HelicsFederateRole` match is an
   unrelated type). The first human build verifies the claim.

## 8. Handoff notes

1. **P13.18 (wave 2)**: the admin role/policy UIs render THIS role model.
2. **P13.19 (wave 2)**: the authz-matrix extension tests assert §3's
   table behaviorally (this record's tests are the in-crate core).
3. **The P13.02 resolver**: `map_role_claims` maps the `site-approver`
   group id through the same closed vocabulary — a SCIM/OIDC group named
   `site-approver` grants approval duty, nothing else.
