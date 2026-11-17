# P13.03 — SCIM 2.0 provisioning design (user lifecycle, group→role mapping, deprovisioning SLA)

| Field | Value |
|---|---|
| Subtask | P13.03 — SCIM 2.0 provisioning design: user lifecycle (create/update/deactivate), group→role mapping onto the `PolicySet` role vocabulary, deprovisioning SLA with audit evidence |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | DESIGN RECORD — no IdP exists, no SCIM client has been exercised, no SCIM object has ever been provisioned, no conformance test has run. Every operational evidence cell is EMPTY/UNRUN. Lab-gated on P13.20; NO conformance claim is made or implied (conventions §7 item 3). |
| Sources | [S-77] (enterprise SSO + Keycloak, gap analysis §11 — the Tier 2 SCIM row); RFC 7643 (SCIM Core Schema) and RFC 7644 (SCIM Protocol) cited as primary standards; `crates/flexcore-authz/src/policy.rs` (the closed `Role` vocabulary — the mapping target) |
| Files touched | none (design record only) |

## 1. Why SCIM and what it does here

SCIM 2.0 is the provisioning protocol enterprise IdPs push user and
group lifecycle events through (create / update / deactivate /
group-membership changes). In FlexCore's architecture (record 01) SCIM
is the WRITE side of the identity data model: the OIDC token validation
(P13.02) is the READ side (who is authenticated right now); SCIM keeps
the tenant's user/group state current between logins. The two consume
the SAME group→role vocabulary so a group's meaning cannot differ
between provisioning and login.

## 2. The user lifecycle (design)

| Event | Design |
|---|---|
| `POST /Users` (create) | The SCIM service creates the user record scoped to the tenant of the SCIM connection (a tenant-data fact, record 01 §4). The record carries the SCIM `id` (stable), `userName`, `active`, `groups`, and the FlexCore role-mapping inputs. No role is granted at creation — roles come ONLY through group membership mapped by the closed vocabulary (§3) |
| `PUT/PATCH /Users/{id}` (update) | Attribute updates apply to the tenant-scoped record; role-affecting fields (`groups`) update the mapping inputs and are audit-recorded as identity facts, never as authorization decisions (the `PolicySet` remains the only decision authority) |
| `DELETE /Users/{id}` or `active=false` (deactivate) | The deprovisioning path (§4). The record deactivates; sessions die at the resolver (P13.16 semantics: a revoked session denies at the resolver, not at the UI) |
| Identity resolution at login | The P13.02 resolver maps token claims; the SCIM state is the store the mapper consults for tenant binding and group membership. A deactivated user's token REJECTS at the resolver (fail closed), regardless of token validity |

## 3. Group→role mapping (the single vocabulary)

| Element | Design |
|---|---|
| The vocabulary | the closed `Role` enum (`flexcore-authz/src/policy.rs`): `platform-admin`, `tenant-admin`, `operator`, `viewer`, and the P13.05 `site-approver` — mapped through their stable ids (the same ids `parse_role` and the OIDC `map_role_claims` consume) |
| The mapping | per-tenant data: SCIM group (displayName or externalId) → role id, plus optional scope subset (site ids for `site-approver`). One group maps to AT MOST one role; an unmapped group contributes nothing; a user in NO mapped group resolves to no role and REJECTS at login (never defaults to Viewer — the P13.02 rule) |
| Never-mappable groups | the never-granted permission set (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) is byte-identical and structurally unreachable: no group maps to a permission outside the closed `Action` surface, and those four names are not on it |
| The single home | the mapping table is defined ONCE (tenant data) and consumed by BOTH the SCIM provisioning path and the P13.02 login path. The P13.19 authz-matrix tests assert the vocabulary is the same set in both paths |

## 4. Deprovisioning SLA and its audit evidence

| Element | Design |
|---|---|
| SLA (REGISTERED RECOMMENDATION, human-gated) | a deactivation received from the IdP is enforced at the resolver within 5 minutes (the propagation window covers SCIM delivery + cache invalidation; a deployment with a longer window records its own number — none is claimed as tested) |
| Enforcement point | the RESOLVER (P13.16 semantics): a deactivated user fails authentication regardless of a still-valid JWT. Token-expiry-only deactivation is explicitly NOT the design (a 15-minute token would outlive the SLA) |
| Audit evidence | every lifecycle event produces audit-record facts: the SCIM event identity (SCIM `id`, tenant), the change (created/updated/deactivated), the mapping inputs before/after (groups), the enforcement instant (when the resolver first denies the user). The evidence lands in the append-only audit trail (the P11.15 keyed chain inherits when keyed) — the deprovisioning proof is a chain of records, not a promise |
| Failure mode | SCIM delivery failure is NOT silent: a failed provisioning event is recorded and retried by the IdP (SCIM is the IdP-driven push); a tenant whose connection goes stale past a registered threshold is flagged. Deprovisioning SLA breach (a deactivated user still authenticating past the window) is an ALARM-class audit fact, never a log line |

## 5. Conformance posture (honest)

SCIM conformance (RFC 7644 compliance) is a human-gated certification
process. This phase prepares: the lifecycle design (§2), the mapping
vocabulary (§3), and the written-unrun conformance scenarios P13.19
names. NOTHING here claims any conformance, and no SCIM object exists —
the lab environment (P13.20) is the gate where a real IdP pushes real
SCIM events.

## 6. ASSUMPTION / UNRUN register

1. **ASSUMPTION** — the SCIM 2.0 protocol/schema descriptions are the
   RFC 7643/7644-documented behavior of the standard; none has been
   exercised against FlexCore.
2. **UNRUN** — no SCIM service exists, no user/group was provisioned,
   no deprovisioning was enforced, no SLA window was measured. Evidence
   cells EMPTY.
3. **REGISTERED, not tuned** — the 5-minute deprovisioning window is a
   pre-registered recommendation (X-8), pending operator sign-off; no
   propagation measurement exists.
4. **OPEN dependence** — P13-E-1 (record 01) stays OPEN: the SCIM design
   implements the DEFAULT architecture; a commercial IdP changes the
   connection mechanics, not the vocabulary or the SLA semantics.

## 7. Handoff notes

1. **P13.19 (wave 2)**: the SCIM conformance scenarios test the §2
   lifecycle and §3 mapping vocabulary (create → login → group change →
   role change → deactivate → resolver deny) — written-unrun.
2. **P13.18 (wave 2)**: the admin users/roles UIs render the §3 mapping
   as tenant data; no write path is fabricated where no endpoint exists.
3. **Agent B**: no webhook/notification artifact consumes SCIM events
   through this record; lifecycle audit facts flow through the audit
   trail like every other evidence record.
