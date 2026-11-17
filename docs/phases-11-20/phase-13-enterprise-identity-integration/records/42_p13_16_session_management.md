# P13.16 — Session management (timeout policy, concurrent-session policy, active-session list design, revocation semantics — denied at the resolver)

| Field | Value |
|---|---|
| Subtask | P13.16 — timeout policy, concurrent-session policy, active-session list design, revocation semantics — a revoked session denies at the RESOLVER, not at the UI |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | WRITTEN-UNRUN: the session module and its tests are written and never compiled or executed; **no session has ever been created, listed, timed out, or revoked anywhere**; no login exists to hold a session (P13.17's flows are themselves written-unrun). Every operational evidence cell is EMPTY/UNRUN. |
| Sources | `apps/control-plane/src/auth.rs` (the `IdentityResolver` seam this composes — consumed, its default rule untouched); `apps/control-plane/src/oidc.rs` (the P13.02 production resolver this wraps in a composed deployment); record 03 §4 (the deprovisioning SLA whose enforcement point IS this module's resolver-layer deny); record 41 §3 D-3 (the tenant-wide offboarding revocation); record 04 (the step-up protocol — a separate, unrelated gate: sessions and step-up challenges are distinct objects); [S-77] (enterprise SSO session practice, the general Tier 2 basis) |
| Files touched | `apps/control-plane/src/session.rs` (NEW), `apps/control-plane/src/lib.rs` (additive module line, marked P13.16) |

## 1. The load-bearing property (the subtask's own sentence)

**A revoked session denies at the RESOLVER, not at the UI.** In this
module that is structural, not aspirational:

- `SessionEnforcingResolver::resolve_session` gates EVERY request:
  inner identity (the token) first, then the session reference, then the
  policy checks (idle, absolute), then the (tenant, subject) binding.
  Any failure is an `AuthError` — the caller never reaches a route
  handler, and the console can only ever display the denial the resolver
  produced.
- Revocation (`revoke_session`, `revoke_all_for_subject`,
  `revoke_all_for_tenant`) is a DIRECTORY write. From the next request
  on, the revoked reference fails authentication even though the bearer
  token may still be cryptographically valid — the test
  `a_revoked_session_denies_at_the_resolver_while_the_token_is_still_valid`
  pins exactly this (valid token + revoked session = 401).
- The UI has no logout authority: "logged out" in the console is a
  DISPLAY of the resolver's denials, never a state the console sets
  (the P13.17 banner renders expiry; it grants and revokes nothing).

## 2. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| Closed session-state vocabulary: `Active` / `Revoked` (extension = a recorded amendment) | `session.rs` | Written-unrun |
| `SessionRecord` bound at creation to a (tenant, subject) pair; the binding re-checked at every resolution (a session reference never rides on another principal's token) | same | Written-unrun |
| Timeout policy: idle timeout (DEFAULT 30 min) + absolute lifetime (DEFAULT 12 h) — both REGISTERED RECOMMENDATIONS (X-8); the idle anchor refreshes ONLY on a fully successful gate (a denied request never extends a session's life — pinned by test) | same | Written-unrun; none of the values is a tested optimum |
| Concurrent-session policy: cap 5 per (tenant, subject), enforced at CREATION (`SessionDirectory::insert` refuses with `SessionLimitReached`); the newest login is DENIED, never a silent eviction of the oldest; independent counters per subject and per tenant | same | Written-unrun |
| `SessionDirectory` port + `InMemorySessionDirectory` (tests / single-process): insert (cap-checked), get, touch, revoke (idempotent; unknown = no-op success), revoke per subject, revoke per tenant, `list_active_for_subject` (the active-session list design surface, ordered by creation; expired-but-unswept rows are not listed — the list shows what would authenticate) | same | Written-unrun; the durable Postgres store (under RLS) is a composition-root artifact — no migration was added |
| `SessionEnforcingResolver`: the session-aware resolution path (`resolve_session(bearer, session_ref)`); the wire contract puts the session reference in the `X-FlexCore-Session` header (the recorded constant `SESSION_HEADER`) | same | Written-unrun |
| Fail-closed wrong-composition shape: the bare `IdentityResolver` trait path cannot see the header, so `resolve` denies unconditionally — composing this resolver into `state.identity` without the session-aware middleware yields a total denial, never a session bypass | same | Written-unrun; pinned by test |
| Denial vocabulary: `SessionDenial` (`NoSession`, `UnknownSession`, `Revoked`, `IdleTimeoutExpired`, `AbsoluteLifetimeExpired`, `IdentityMismatch`, `SessionLimitReached`, `StoreError`), each with readable audit text | same | Written-unrun |
| Tests: create/resolve/touch; bare-trait fail-closed; missing/unknown/blank session reference; revoked-denies-with-valid-token + idempotent revocation; idle timeout + no-extension-on-deny; absolute lifetime over activity; binding mismatch (subject); cap refusal + independent counters; tenant-wide revocation scoped to the tenant; list shows only live sessions; empty subject never creates | `session.rs` `#[cfg(test)]` | Written complete; UNRUN (conventions §1 item 3 — execution is the human gate) |

## 3. The timeout / concurrency policy table (registered, X-8)

| Policy | DEFAULT | Anchor | Enforcement point | Change procedure |
|---|---|---|---|---|
| Idle timeout | 30 min | last successful authenticated use | the resolver gate (deadline = last_seen + idle) | recorded amendment to this record |
| Absolute lifetime | 12 h | session creation | the resolver gate (deadline = created + lifetime); activity never extends it | recorded amendment |
| Concurrent sessions | 5 per (tenant, subject) | login | `SessionDirectory::insert` refuses the NEW session (no silent eviction) | recorded amendment |
| Revocation | immediate | the revocation call | the resolver gate on every later request; no grace window | the semantics are the design; a deployment may not loosen them |

## 4. The active-session list design (what P13.18 renders)

`list_active_for_subject(tenant, subject)` returns the sessions that
would authenticate right now (ACTIVE, unexpired, of the pair), ordered by
creation. The Administration UI design (record 44) renders it with
created-at / last-seen-at / state columns and a revoke action per row.
The REST endpoint that would serve the list and the revocation
(`GET /api/v1/sessions`, `DELETE /api/v1/sessions/{id}` — or whatever
shape a future implementation gives them) **does not exist** and is
recorded specified-not-implemented (plan risk 6): no route, no handler,
no DTO was fabricated. The list method is a library seam, not an HTTP
surface.

## 5. Composition honesty (what was deliberately NOT edited)

| File | State | Why |
|---|---|---|
| `apps/control-plane/src/main.rs` | untouched | no IdP exists; no production identity path exists to wrap; composing a session resolver that wraps nothing would be dishonest wiring (the Agent A wave-1 precedent for `main.rs`) |
| `apps/control-plane/src/state.rs` | untouched | `AppState.identity` stays `DenyAllResolver`; a session-enforcing resolver cannot even be composed into the trait slot without the session-aware middleware that carries the header — wiring it wrong would fail closed on every request, which is a deployment decision, not a code edit this batch makes |
| `apps/control-plane/src/auth.rs` | untouched | the resolver seam and its default rule are consumed exactly as they are; the additive extension point is the new module, not the seam |
| `apps/control-plane/src/oidc.rs` | untouched | the P13.02 resolver is the WRAPPED inner path (consumed by citation and by generic `Box<dyn IdentityResolver>`); no line of it changes |

## 6. The never-granted permission set and the frozen sets — byte-identical

Quoted unchanged (conventions §6.4): `dispatch.authorize`,
`dispatch.issue`, `secret.read`, `network.any`. Nothing in this module
grants anything to anyone: a session is an authentication gate, not an
authorization grant — the `PolicySet` evaluation happens exactly as
before, AFTER this resolver produces the `Caller`. The frozen 14-state /
27-edge command lifecycle and the frozen 26-event vocabulary are
untouched (no file of either was opened for edit).

## 7. ASSUMPTION / UNRUN register

1. **UNRUN** — every function and test; no session exists anywhere; no
   revocation has ever been enforced; no list has ever been rendered.
   Evidence cells EMPTY.
2. **REGISTERED, not tuned** — 30 min idle, 12 h absolute, cap 5 (X-8
   pre-registration; none is a tested optimum; all are recorded
   amendments away from change).
3. **ASSUMPTION** — the `X-FlexCore-Session` header contract is this
   module's recorded wire shape; a deployment may instead bind sessions
   to the token's IdP `sid` claim — that variant is a composition choice
   this record notes but does not build (the header path is the one
   implemented).
4. **RECORDED LIMITATION** — the in-memory directory is single-process;
   a multi-instance deployment needs the durable store (Postgres under
   RLS) or sticky routing; neither is built or claimed.
5. **RECORDED LIMITATION** — expired sessions are denied lazily (at
   resolution) and swept lazily (filtered from the list); there is no
   background sweeper. A deployment wanting proactive sweep owns it at
   composition.

## 8. Handoff notes

1. **P13.17 (record 43)**: the console consumes the denial semantics
   (401 ⇒ the session is dead; the banner displays expiry) and the
   `X-FlexCore-Session` header contract; the console never asserts
   session authority.
2. **P13.18 (record 44)**: the active-session list design (§4) is the
   management-UI seam; the endpoint is specified-not-implemented there.
3. **P13.19 (record 45)**: the resolver-level negative tests (revoked /
   expired / binding mismatch) are extended at integration level in the
   suite's session slice.
4. **P13.15 (record 41)**: D-3 consumes `revoke_all_for_tenant` — the
   cross-reference is recorded both sides.
