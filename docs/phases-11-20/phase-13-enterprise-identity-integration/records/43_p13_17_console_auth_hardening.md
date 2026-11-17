# P13.17 — Console auth hardening (real login flow design, step-up UX on the enforced protocol, session banner with expiry display)

| Field | Value |
|---|---|
| Subtask | P13.17 — real login flows against the IdP (redirect/callback design), step-up UX wired to the P13.04 enforced protocol, session banner with expiry display |
| Wave / agent | Phase 13 wave 2, Agent A |
| Status | WRITTEN-UNRUN throughout: **no login has ever been performed, no IdP redirect has ever happened, no callback has ever been handled, no session has ever been established, no step-up challenge has ever been issued from the console, no banner has ever rendered in a running console** (the console loop bans install/build; the human gate runs `npm test` after the first install). Every operational evidence cell is EMPTY/UNRUN. |
| Sources | record 02 (the P13.02 OIDC design — the server-side confidential-client exchange this flow consumes; consumed by citation); record 04 (the P13.04 enforced step-up protocol — consumed by citation, never redefined); record 42 (the P13.16 session semantics the banner displays); report §20.2 (explicit confirmation / the display principles); plan risk 6 (the specified-not-implemented discipline); the console conventions (`console/src/components/stepUpApproval.tsx` honesty-record pattern, `state/tenantContext.ts` store pattern, `components/tenantBanner.tsx` shell-display pattern, `components/stepUpApproval.test.tsx` test pattern incl. the structural source scan) |
| Files touched | `console/src/auth/session.ts` (NEW), `console/src/auth/oidcLogin.ts` (NEW), `console/src/auth/stepUpFlow.ts` (NEW), `console/src/components/sessionBanner.tsx` (NEW), `console/src/auth/session.test.ts` (NEW), `console/src/auth/oidcLogin.test.ts` (NEW), `console/src/auth/stepUpFlow.test.ts` (NEW), `console/src/components/sessionBanner.test.tsx` (NEW), `console/src/App.tsx` (ADDITIVE composition-root edit, marked P13.17: one import + one `<SessionBanner />` render next to the P5.32 TenantBanner — no existing line modified) |

## 1. The real-login flow (DESIGN — specified-not-implemented endpoints)

The console never holds credentials, never builds the authorize request,
and never sees a token. The flow is server-mediated (the confidential
client of records 01/02); the console's part is navigation + display:

| Step | Actor | Endpoint | State |
|---|---|---|---|
| 1 | console navigates to the login entry | `GET /api/v1/auth/login?return_to=<path>` | **SPECIFIED-NOT-IMPLEMENTED** |
| 2 | control plane builds the OIDC authorize request (PKCE + `state` + `nonce` are SERVER-side construction) and redirects to the per-tenant IdP connection | (the IdP's authorize endpoint) | the IdP exists only in the P13.20 lab spec — UNRUN |
| 3 | IdP authenticates (P13-E-3 DEFAULT: IdP-delegated MFA) and redirects | `GET /api/v1/auth/callback` (control-plane-hosted) | **SPECIFIED-NOT-IMPLEMENTED** |
| 4 | control plane validates + exchanges the code server-side, maps claims through the P13.02 vocabulary (ONE closed role vocabulary), creates the session per P13.16 | (server-internal) | the resolver and session modules are written-unrun |
| 5 | control plane redirects back to the validated `return_to` | console landing | the `return_to` guard (`validateReturnTo`) is console-side design; the SERVER re-validates independently — neither trusts the other's check |
| 6 | console reads the session facts and the authentication layer sets BOTH stores (claims → `tenantContext`, lifecycle → `auth/session`) | `GET /api/v1/auth/session` | **SPECIFIED-NOT-IMPLEMENTED** |

Until step-6's endpoint exists, no store is written and the console
renders the honest no-session / lifecycle-unknown states. The three
endpoint paths are checked-in constants (`oidcLogin.ts`) — the recorded
design surface, with `LOGIN_FLOW_NOT_IMPLEMENTED_NOTE` rendered where the
UI names them.

## 2. What was implemented (written, never run)

| Piece | Where | Honest status |
|---|---|---|
| Session-lifecycle store: `ConsoleSessionLifecycle` (session reference + absolute expiry), get/set/clear/subscribe mirroring the `tenantContext.ts` pattern; claims deliberately NOT duplicated (they stay in the P5.01 store) | `console/src/auth/session.ts` | Written-unrun; nothing sets it until a deployment composes the login flow |
| Pure expiry display: `formatSessionExpiry` — active / expiring-soon (REGISTERED RECOMMENDATION threshold: 5 min, X-8) / expired, text-borne (never colour alone); the expired text names the RESOLVER as the enforcer | same | Written-unrun; a stale client clock can misrender the countdown and the text says so — it can never extend a session |
| Login-flow design: the six-step record (§1), the strict `validateReturnTo` open-redirect guard, `buildLoginUrl`, `parseLoginLanding` (reads ONLY the recorded `login_error` parameter — unrecorded query content never derives console state) | `console/src/auth/oidcLogin.ts` | Written-unrun; no fetch, no navigation (structural test pins it) |
| Enforced-protocol step-up flow: the closed denial vocabulary (the nine record-04 `StepUpDenial` variants, mirrored exactly), the closed purpose vocabulary (`StepUpPurposeView`), the five-stage machine (`idle → challenge-issued → verifying → verified \| denied`) driven ONLY by server-supplied results; an unrecognized server denial renders fail-closed; a second verdict after verified renders the replay denial; no code path produces `verified` without a server ok (structural test pins the single construction site) | `console/src/auth/stepUpFlow.ts` | Written-unrun; the challenge/verify endpoints (`/api/v1/auth/step-up/challenge`, `/api/v1/auth/step-up/verify`) are SPECIFIED-NOT-IMPLEMENTED |
| Session banner: the persistent shell display — no-session state with a DISABLED Sign-in control (no navigation to a nonexistent path), lifecycle-unknown state, expiry line (the three buckets), the session-reference display, a "Clear the local display" control described honestly as display-only (never a logout) | `console/src/components/sessionBanner.tsx` | Written-unrun |
| Composition: `<SessionBanner />` rendered in the shell next to the P5.32 TenantBanner | `console/src/App.tsx` (additive, marked) | Written-unrun |
| Tests: store round-trip/subscription; expiry boundary pinning (inclusive expired boundary, exact 5-min window); the return-to guard's accept/reject matrix; landing-parser strictness; the step-up machine's full transition matrix (idle/challenge/verifying/verified/denied, replay, unrecognized denial, invalid-transition fail-closed); banner rendering per state; four structural source scans (no fetch / no navigation in the auth modules and the banner) | the four test files | Written complete; UNRUN (conventions §1 item 3) |

## 3. The P5.26 UI-only flow's fate (record 04 §4, honored)

`stepUpApproval.tsx` is NOT edited. Its honesty record ("LOCAL UI EVENT —
NOT a server-verified step-up") remains true for that component, and the
new `stepUpFlow.ts` module carries the counterpart note
(`STEP_UP_ENFORCED_NOTE`): the two are never interchangeable, and the
tests assert the enforced note says "NOT the P5.26 local UI event". The
coexistence is deliberate (the P7.25 additive precedent): the composition
that wires a gated action through the enforced flow will retire the
UI-only path at that point — this batch retires nothing, edits nothing
existing, and replaces nothing's honesty record.

## 4. Specified-not-implemented register (plan risk 6)

| Endpoint / capability | State |
|---|---|
| `GET /api/v1/auth/login` | does NOT exist — designed (§1 step 1) |
| `GET /api/v1/auth/callback` (control-plane-hosted) | does NOT exist — designed (§1 step 3) |
| `GET /api/v1/auth/session` | does NOT exist — designed (§1 step 6); feeds `setSessionLifecycle` |
| `POST /api/v1/auth/step-up/challenge` and `POST /api/v1/auth/step-up/verify` (the HTTP surface of record 04 §2) | do NOT exist — the flow machine consumes supplied results; no fetch exists |
| Sign-in / sign-out controls | rendered DISABLED or as honest notes; no navigation to a nonexistent path anywhere |

## 5. ASSUMPTION / UNRUN register

1. **UNRUN** — every module, test, and rendering. No login, no session,
   no challenge, no verdict, no banner in a running console. Evidence
   cells EMPTY.
2. **REGISTERED, not tuned** — the 5-minute expiring-soon display
   threshold and the 30-second banner tick (X-8); both are display
   parameters, never enforcement.
3. **ASSUMPTION** — the six-step flow shape (confidential client,
   server-side exchange, session created at the control plane) is the
   record 01/02 architecture's console projection; a deployment choosing
   a public-client SPA flow would change steps 2–4 — that variant is not
   designed here (the confidential-client shape is the recorded one).
4. **RECORDED LIMITATION** — the expiry display shows the ABSOLUTE
   deadline only; the P13.16 idle timeout can end the session earlier,
   which the console cannot see (the banner text states this verbatim).
5. **RECORDED LIMITATION** — `Date.now()` drives the countdown; a skewed
   client clock misrenders the display (never the enforcement — the
   text states it).

## 6. Handoff notes

1. **P13.18 (record 44)**: the Administration section composes additively
   below the P5.07 workspace (the same P7.25 pattern this batch used for
   the banner); no existing workspace file is edited.
2. **P13.19 (record 45)**: the step-up negative matrix extends record 04's
   server-side surfaces; the console flow tests here are the display-side
   counterpart, kept UNRUN.
3. **P13.20 (record 46)**: the lab compose profile is where the §1
   endpoints' first real exercise happens (the human gate); until then
   every step is UNRUN.
4. **Human gates**: the first console install (runs `npm test` over the
   four new test files); the server endpoint implementation (a later
   batch's work — the constants here are its recorded contract surface).
