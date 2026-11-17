# P13.08 — API-key issuance/rotation service design + the webhook signing-secret FORMAT (defined once, consumed by P13.09)

| Field | Value |
|---|---|
| Subtask | P13.08 — the legacy/simple integration tier: per-tenant API keys with rotation and last-used evidence; scopes a strict subset of the typed authorization vocabulary; HMAC webhook secrets bound per key; storage follows the P11.16 secrets posture. **Defines the webhook secret FORMAT once — P13.09 (Agent B) consumes it by citation, never redefines it.** |
| Wave / agent | Phase 13 wave 1, Agent A |
| Status | DESIGN RECORD — no key is issued, no key exists, no secret is generated, no webhook secret is stored anywhere, no API call has ever authenticated with one. Every operational evidence cell is EMPTY/UNRUN. |
| Sources | P11.16 (forward seam: `records/31_p11_16_secrets_management.md` under phase-11 — Vault DEFAULT / Compose file-per-host floor, consumed, never re-decided); `crates/flexcore-authz/src/action.rs` (the closed typed `Action` vocabulary — the scope space); conventions §6.4 (the never-granted permission set stays byte-identical); plan §2.3 (the webhook delivery contract — P13.09's, referenced not redefined) |
| Files touched | `crates/flexcore-authz/src/action.rs` (additive: the `apikey.*` action variants — see record 05 for the full closed-enum extension); design only otherwise |

## 1. Position in the identity architecture

API keys are the **legacy/simple tier** of machine identity (record
`07_p13_07_machine_identity.md` is the OAuth2 client-credentials tier for
grid-operator and Phase 16 market-adapter integrations). An API key is a
tenant-scoped credential for simple server-to-server integrations that do
not need a full OAuth2 client: it authenticates a *service principal in
one tenant*, never a human, never a platform operator, never an edge
device (edge identity stays Phase 18's; never re-decided here).

| Principal class (P11.14 axes) | Credential | Records |
|---|---|---|
| Human users | OIDC via the per-tenant SSO connection (P13.01/P13.02) | 01, 02 |
| Grid-operator / market integrations | OAuth2 client-credentials JWT | 07 |
| Simple per-tenant integrations (this record) | Bearer API key | 08 |
| Edge agents | Phase 18 device identity (NOT an API key) | — |

## 2. The API-key model (design)

| Element | Design |
|---|---|
| Key ID | `fkid_<10 lowercase base32 chars>` — public, travels in audit records, safe to log |
| Key secret | `fk_<43 unpadded base64url chars>` — 32 bytes of entropy from a CSPRNG, rendered per the same unpadded-base64url rule as the webhook secret (§4); shown ONCE at creation and at each rotation, never stored retrievable |
| At-rest storage | the SHA-256 digest of the full secret string is stored in the tenant-scoped key table (key id, digest, tenant, scopes, state, rotation fields, last-used evidence); the plaintext never persists in the database. Any deployment-secret material (e.g. an offline recovery escrow, if an operator wants one) lands in the P11.16 posture — Vault DEFAULT, Compose file-per-host floor — never `.env`, never compose-inline |
| Scopes | a STRICT SUBSET of the typed `Action` vocabulary (the closed enum in `crates/flexcore-authz/src/action.rs`), expressed as canonical action ids. The never-granted names (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`) are structurally excluded: they are not on the `Action` surface at all (parse → `None` → `UnknownAction` deny), so no scope string can ever re-introduce them |
| Scope check | a key-scoped request is evaluated through the existing deny-by-default `PolicySet` path: the key's scopes grant the principal a service role at tenant scope; an action outside the key's scope list is a reasoned deny (recorded) |
| Rotation | rotate = issue a new secret for the same key id, mark the old secret `superseded` with an overlap window (default: 24 h, a REGISTERED RECOMMENDATION, not a tuned claim) during which both digests verify; the old digest is revoked when the overlap closes; rotation is itself an audited action |
| Revocation | immediate: state → `revoked`; the resolver consults the key table on every use — there is no "works until its next refresh" window for a revoked key |
| Last-used evidence | per key: `last_used_at`, `last_used_subject_path`, `last_used_result` (allow/deny), and a monotonically increasing use counter — updated on use; a key unused past a registered threshold (RECOMMENDATION: 90 days) is flagged for review, never auto-revoked |
| Audit | creation, rotation, revocation, scope change, and denial-vs-scope evidence are audit-record facts (the flexcore-audit append-only trail; the P11.15 keyed chain inherits when keyed) |

## 3. The `apikey.*` action surface (additive, closed-enum extension rule)

New actions extend `Action` ADDITIVELY (a variant, its `id` arm, its
`parse` arm, and explicit policy grants — nothing matches by default).
Added in this subtask (implemented in `crates/flexcore-authz/src/`,
recorded in detail in record 05):

| Action id | Granted to (RECOMMENDED DEFAULT, reviewable) | Scope kinds |
|---|---|---|
| `apikey.create` | TenantAdmin (own tenant), PlatformAdmin | tenant |
| `apikey.rotate` | TenantAdmin (own tenant), PlatformAdmin | tenant |
| `apikey.revoke` | TenantAdmin (own tenant), PlatformAdmin | tenant |
| `apikey.read` | TenantAdmin (own tenant), PlatformAdmin | tenant |

Deliberately NOT added here: any `webhook.*` management action (Agent B's
P13.09 defines the webhook endpoint model; if its management surface
needs actions, they extend the enum the same additive way — B proposes
them, the authz crate's closed-enum extension rule applies verbatim) and
any key-authenticated write beyond what the key's scope subset names.

## 4. THE WEBHOOK SIGNING-SECRET FORMAT (the P13.08 → P13.09 seam — defined ONCE, here)

Agent B's P13.09 consumes this format BY CITATION. It is not restated,
renamed, or re-derived anywhere else.

| Element | The format |
|---|---|
| Literal form | `whsec_<43 unpadded base64url characters>` — total length 49 |
| Entropy | exactly 32 bytes from a CSPRNG, base64url-encoded WITHOUT padding (32 bytes → 43 characters) |
| Parsing rule | strict: the `whsec_` prefix is exact; the remainder must be exactly 43 characters of the unpadded base64url alphabet (`A–Z a–z 0–9 - _`); it MUST decode to exactly 32 bytes. Anything else — wrong prefix, wrong length, padding characters (`=`), non-alphabet characters, wrong decoded length — is a named parse rejection, never a lenient accept |
| Binding | exactly one live secret per webhook endpoint at rest (B's endpoint model); during P13.09's rolling overlap an endpoint carries `current` + `previous` — both of the SAME format above |
| Computation | the signature computation over `timestamp + "." + rawbody` is plan §2.3 / [S-83] territory — P13.09's signer. This record defines only the SECRET MATERIAL |
| Generation | generated by the API-key service at endpoint creation and at each rotation; displayed ONCE to the tenant admin; never logged, never echoed in a GET response (responses carry the key/endpoint id and a secret fingerprint — the first 8 characters after the prefix — never the secret) |
| Storage | the secret lands in the P11.16 posture (Vault DEFAULT; Compose file-per-host floor at minimum), not in the tenant database. The database row carries a fingerprint and a state, never the material |

RATIONALE (recorded, not negotiable at wave level): the format is defined
once because two definitions (even identical ones) are two facts that can
drift; the gap-analysis §9 rule and the plan §2.1 ordering rule both name
P13.08 as the definition point and P13.09 as the consumer.

## 5. Interaction with the never-granted set and the command lifecycle

The never-granted permission set is **byte-identical** after this
subtask: `dispatch.authorize`, `dispatch.issue`, `secret.read`,
`network.any` — quoted here verbatim as the four names, none of which
exists on the `Action` surface, none of which any API key can request as
a scope, and none of which this record adds. An API key reaches dispatch
reach only through the command lifecycle (the P4.11–P4.13 gates), the
same as every other principal (conventions §6.4).

## 6. ASSUMPTION / UNRUN register

1. **UNRUN** — no key generated, no digest stored, no rotation executed,
   no last-used evidence written, no webhook secret created or stored.
   All evidence cells EMPTY.
2. **ASSUMPTION** — the 24 h rotation overlap and the 90-day unused-key
   review threshold are REGISTERED RECOMMENDATIONS pending operator
   sign-off (X-8 discipline), never tested defaults.
3. **ASSUMPTION** — CSPRNG availability at the deployment host (the
   standard OS entropy source) is a deployment requirement; nothing in
   this repo generates material at design time.
4. **RECORDED LIMITATION** — digest-at-rest means the plaintext cannot be
   recovered to re-display; that is the point. A lost key is rotated, not
   recovered.
5. **OPEN DEPENDENCE** — storage posture consumes the P11.16
   RECOMMENDATION (not deployed); if the human owner declines Vault, the
   Compose file-per-host floor applies, never a database column.

## 7. Handoff notes

1. **Agent B (P13.09)**: cite §4 for the secret format and plan §2.3 for
   the signature computation. Do not add a second format definition. Your
   endpoint model binds `current`/`previous` secrets per the overlap
   semantics of §2; the format is invariant across rotation.
2. **Wave 2 (P13.18)**: the admin UI renders key id + fingerprint +
   scopes + last-used evidence; creation/rotation responses display the
   secret exactly once.
3. **P13.19 (wave 2 tests)**: the scope-subset rule and the never-granted
   exclusion are the test targets (a key scope string naming
   `dispatch.authorize` must parse-deny, structurally).
