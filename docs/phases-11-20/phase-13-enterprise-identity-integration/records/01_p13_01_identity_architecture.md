# P13.01 — Identity architecture decision record (OIDC-first; SAML via broker; per-tenant SSO connections as tenant data)

| Field | Value |
|---|---|
| Subtask | P13.01 — the identity architecture decision record: OIDC-first with SAML supported via an OIDC broker; Keycloak DEFAULT vs a commercial IdP; per-tenant SSO connections as tenant data; registered as **P13-E-1** |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | DESIGN RECORD — nothing is deployed, configured, or connected. No IdP exists in this repo or any lab; no OIDC discovery document has been fetched; no SAML metadata has been exchanged; no tenant SSO connection has been created. Every operational evidence cell is EMPTY/UNRUN. |
| Sources | [S-77] (enterprise SSO providers + Keycloak, gap analysis §11); LR-047 (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` §3.8: "No IdP integration exists (only DenyAllResolver/dev resolver) — identity enforcement is future composition"); RFC 6749 (OAuth 2.0), RFC 7519 (JWT), OpenID Connect Core 1.0 (primary standards, cited as standards — see the ASSUMPTION register) |
| Decision register | **P13-E-1 stays OPEN.** This record registers the question, the options, and the DEFAULT/RECOMMENDATION. Agents never close an E-decision (conventions §4); the human owner closes it on budget and vendor judgment. |

## 1. The decision question (P13-E-1, registered OPEN)

| Element | Content |
|---|---|
| Question | Which identity-provider architecture does FlexCore's production identity path standardize on: a self-hosted open-source IdP (Keycloak), a commercial IdP (Okta/Entra/Auth0 class), or both? |
| Options | (a) self-hosted Keycloak; (b) commercial IdP only; (c) both — Keycloak as the reference/lab IdP, commercial IdPs supported per tenant |
| DEFAULT / RECOMMENDATION | **OIDC-first with Keycloak as the reference IdP and lab target** (option c's shape) [S-77]. Commercial IdPs remain supported as per-tenant SSO connections — the architecture below makes the IdP a tenant-level fact, not a platform commitment. Budget and vendor selection are the human owner's call. |
| Closer | human owner |
| Status | **OPEN — never closed by this record or by any agent.** P13.02 (middleware) and P13.20 (lab spec) implement the DEFAULT and record the dependence per conventions §4. |

## 2. The architecture decision (the DEFAULT, implemented-not-deployed)

| Decision | Content | Rationale |
|---|---|---|
| OIDC-first | OpenID Connect (the identity layer over OAuth 2.0) is the federation protocol FlexCore validates natively: authorization-code flow for interactive users at the IdP, JWT access/ID tokens validated server-side by the P13.02 middleware (record `02_p13_02_oidc_token_validation.md`) | OIDC is the protocol the Tier 2 enterprise-SSO research centers on [S-77]; it composes with the existing seam (`IdentityResolver` in `apps/control-plane/src/auth.rs`) without a protocol bridge |
| SAML via an OIDC broker | SAML is NOT validated natively anywhere in FlexCore. A tenant whose IdP speaks SAML connects through an OIDC-broker deployment (a broker instance, e.g. a Keycloak realm acting as SAML-to-OIDC broker, performs the SAML exchange and issues an OIDC token FlexCore validates) | one validation path (P13.02) stays a single audited surface; native SAML support would duplicate the highest-risk code path in the platform; the broker pattern is the documented SAML-onboarding shape for OIDC-first stacks [S-77] |
| Per-tenant SSO connections are TENANT DATA | Each tenant's IdP connection (issuer URL, client id, claims mapping, connection state) is a row scoped to that tenant under the platform's tenancy rules (RLS posture, [S-70] inherited) — not platform config, not a compiled-in IdP, not a per-deployment file | tenants onboard and change IdPs without a platform release; cross-tenant isolation of identity configuration follows the same RLS discipline as every other tenant-owned row; a revoked/disabled connection is a data-state change with audit evidence |
| Machine principals | Service/machine identity is NOT handled by interactive SSO: OAuth2 client-credentials JWT per record `07_p13_07_machine_identity.md`; legacy/simple integrations use per-tenant API keys per record `08_p13_08_api_keys_webhook_secret_format.md` | distinct principal classes with distinct credential lifecycles; the P11.14 principal classes (Operator / EdgeAgentUplink / Plugin) remain the budget axis |
| Step-up/MFA | IdP-delegated: the IdP owns the authenticator. Registered as **P13-E-3** (record `04_p13_04_step_up_mfa.md`) — also OPEN | FlexCore never becomes an MFA store; the step-up protocol consumes IdP authentication evidence |

## 3. What LR-047 closure means here (honest scope)

LR-047 records that identity enforcement is "future composition" of the
existing `IdentityResolver` seam (`apps/control-plane/src/auth.rs`:
`DenyAllResolver` default, `DevAuthResolver` behind the non-default
`dev-auth` feature). This phase closes LR-047 **as designed and
implemented-written-unrun**: P13.02 provides the production resolver that
composes the seam. LR-047 is closed as *implemented, awaiting human
build/test* — nothing more. Specifically NOT claimed by this record:

- No IdP is reachable from any code path in this repo.
- No tenant SSO connection exists (no tenants exist beyond the design
  baseline).
- No claim that Keycloak (or any broker) "works with FlexCore" — that is
  the P13.20 lab environment's human gate.
- `DenyAllResolver` remains the shipped default resolver; the production
  resolver is composed per deployment, never silently enabled.

## 4. The identity data model (design; nothing provisioned)

| Object | Home | Notes |
|---|---|---|
| SSO connection | tenant-scoped row: `tenant_id`, `protocol` (`oidc` — the only natively validated value), `issuer`, `client_id`, `claims_mapping`, `state` (`enabled` / `disabled`), audit fields | `protocol` is a closed vocabulary; a SAML tenant is represented by its broker's OIDC connection, never by a native `saml` value |
| Claims mapping | per-connection mapping of IdP claims → FlexCore identity facts (tenant binding, subject, group→role via the P13.03 vocabulary) | resolved at the P13.02 middleware from the verified token, never from request fields (P0.16 rule 5) |
| Federation session | the P13.16 session design (wave 2 consumes this record) | sessions reference the SSO connection used; revocation denies at the resolver |
| Machine clients / API keys | records 07 and 08 respectively | never share the interactive-SSO table |

## 5. ASSUMPTION / UNRUN register

1. **ASSUMPTION** — the OIDC/SAML protocol descriptions here are the
   OpenID Connect Core 1.0 / RFC 6749 / RFC 7519 / SAML 2.0-standard
   documented behavior of those standards and of the [S-77]-cited tools;
   none has been exercised against FlexCore.
2. **ASSUMPTION** — Keycloak's SAML-to-OIDC broker capability is the
   [S-77]-documented tool behavior; it is the basis of the "SAML via
   broker" decision and is verified only at the P13.20 lab gate.
3. **UNRUN** — everything operational: no discovery fetch, no token
   validation, no connection row created, no broker stood up. Evidence
   cells: EMPTY.
4. **OPEN** — P13-E-1 (this record §1) and P13-E-3 (record 04). Neither
   is closed by an agent.

## 6. Handoff notes

1. **Agent B / wave 2**: consume this record's decisions by citation;
   never restate the architecture in a second home. The tenant-data
   location of SSO connections is the P13.15/P13.18 consumption point.
2. **P13.02** implements the DEFAULT (OIDC validation middleware) and
   records its dependence on this OPEN E-decision (conventions §4).
3. **Human owner**: closing P13-E-1 requires a budget/vendor sign-off;
   until then every downstream artifact stays DEFAULT-implemented,
   nothing deployed.
