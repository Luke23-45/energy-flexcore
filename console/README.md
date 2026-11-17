# Neryva FlexCore — Operator Console

**Status: complete source, WRITTEN-UNRUN.** Nothing in this directory has been
installed, built, type-checked, linted, or executed — the Phase 5 loop
(`ledger/phase-05-operator-console-simulator/`, decision record
`01_25_scaffold_live.md`) bans installs, builds, and test runs, and the ban
held through every later wave. Every file is complete source written against
the pinned toolchain; the first human install/build/test pass is the
verification gate (see "Verification gates" below).

## What this is

The operator console for the Neryva FlexCore platform: a **static bundle**
(Vite + React + TypeScript, report §7.5, §21.4) served by the control plane or
a CDN. It is a **view** of the simulation-only 0.x platform — it never issues
device commands, never defines authoritative permissions, and never displays a
simulated value as measured (§7.5, §20.2). Control authority in this phase is
Simulation / read-only (shadow) only (§21.4); the mode indicator reads a
deployment constant, never user input (`src/security/modeControl.ts`).

The console is **read-only against the platform**: every call the API client
issues is a `GET` (`src/api/client.ts`). There is no write path and no
execute-command client — the server has no generic execute endpoint (§15.2)
and the console adds none. The one console-triggered high-impact action
(command approval in Dispatch) runs through the step-up flow and
`ConfirmHighImpact`, and only gates the console's own UX — enforcement is the
server's.

## Shipped surface

### The twelve §20.1 workspaces (all implemented)

Overview, Portfolios, Sites, Assets, Grid, Schedules, Dispatch,
Market & Programs, Simulation Lab, Plugins & Integrations, Audit & Reports,
Administration — the registry (`src/workspaces/registry.ts`) is the single
source of truth, in report order. Each workspace renders tenant-scoped API
reads with honest empty/error/loading states: a missing datum states WHY it is
missing, an API error is shown verbatim, and nothing is filled (§20.2).
No demo data exists anywhere in the console.

Additive workspace extensions (the new-file composition precedent — the base
workspace component is not modified):

- **Sites edge status** (`SitesEdgeStatus.tsx`, P6.25): edge connectivity with
  last-seen facts; a missing report renders the gap, never a plausible
  "connected".
- **Plugin inventory** (`PluginsWorkspace.tsx`, P7.25): plugin id/version/
  class, requested permissions, contracts, license, lifecycle vocabulary —
  rendered verbatim; config shows property names/types, never values (§5.7).
- **Research status** (`ResearchWorkspace.tsx`, P8.27): every research
  module's recorded status — the closed vocabulary has no "results exist"
  value; evaluation-record gaps show `TEMPLATE_EMPTY`/`NO_RECORD`.
- **Administration management catalog** (`AdminManagementSection.tsx`,
  P13.18): each management action NAMES the REST endpoint it would call and
  is marked specified-not-implemented — no control fabricates a call.

### Shell chrome

- Control-authority banner (§21.4 simulation/read-only text).
- Tenant banner (`components/tenantBanner.tsx`, P5.32): claims-derived tenant
  + role only; no tenant selector exists anywhere; the server derives tenancy
  from authenticated claims (§16.2).
- Session banner (`components/sessionBanner.tsx`, P13.17): session lifecycle
  display with expiry states. DISPLAY ONLY — a revoked or expired session
  denies at the resolver; the banner cannot extend, revive, or revoke one.
  The sign-in affordance is DISABLED: the real login flow (`auth/oidcLogin.ts`)
  is specified-not-implemented.
- Control-authority mode indicator + mode-gate statement
  (`security/modeControl.ts`, P5.27).
- Value-type legend (P5.10) and live-update status line (P5.25), persistent on
  every workspace.

### Auth / step-up (console-side, P13.17)

- `auth/session.ts` — the session lifecycle store the display reads.
- `auth/stepUpFlow.ts` — the step-up UX state machine consuming the P13.04
  ENFORCED server protocol by citation (closed purpose/denial vocabularies
  mirrored as views). The machine can never reach `verified` without a server
  verdict; single-use/time-bound semantics are the server's.
- `components/stepUpApproval.tsx` — the Dispatch approval panel wired to that
  machine; local denials (capability + step-up) are recorded in
  `security/denialLog.ts` (a local diagnostic log, not an enforcement
  surface).
- `security/capabilityMatrix.ts` + `security/useCan.ts` — the recorded grant
  mirror behind the UX gates; visibility is enforced by the API, and a
  forbidden read renders as an error, never as an empty dataset.

### Live stream (P5.25 / P5.33) — with the re-sync contract wired

- `live/sseClient.ts` — fetch-based SSE consumer for `GET /api/v1/live`
  (the Authorization header requirement rules out native `EventSource`;
  a token in the URL query would be a credential leak), with reconnect
  backoff and the honest disconnect contract: "live updates paused — data may
  be stale", never a silent continue.
- `live/ordering.ts` — client-side ordering and deduplication (P5.33,
  IMPLEMENTED): per-entity last-write-wins by `event_time`, dedup by identity,
  contract-defect and tenant-mismatch drops, all counted in observable
  diagnostics.
- `live/useLiveEvents.ts` — the session hook: status, last delivery time, the
  ordering gate's last-applied envelope, and the gap flags (`needsResync`,
  `resyncToken`).
- **Re-sync, wired end to end**: every stream gap (disconnect into
  paused-stale, a `RESYNC_REQUIRED` notice, a reconnect) bumps the session's
  monotonic `resyncToken`. The shell provides the token to the mounted
  workspace through `WorkspaceResyncContext` (`components/workspaceData.tsx`);
  every workspace data hook re-fetches its read from the REST APIs on a token
  change and acknowledges the re-sync once the fresh read lands. A FAILED
  re-read never acknowledges — the gap stays flagged, and nothing is filled
  silently (§15.1). Operator-run query results (Audit & Reports) are re-run by
  the operator, not re-run silently behind their back.
- Live dispatch events are received, ordered, and deduplicated; applying an
  individual event to a displayed lifecycle badge inline is the recorded next
  integration point (`LIVE_DISPATCH_LABEL_NOTE` in
  `components/honesty.tsx` states exactly this).

### Honesty surfaces (§20.2/§21.4 vocabulary)

`components/honesty.tsx` and `components/stateVocabulary.ts` carry the
single-source treatments: the 14-state command lifecycle with its
SIMULATED qualifications (issuance targets the simulator; acknowledgement is
application-layer acceptance, never physical delivery, §14.1), the six
telemetry quality states (`GOOD`/`SUSPECT`/`BAD`/`MISSING`/`ESTIMATED`/`STALE`),
the §12.6 solver outcomes, and the verbatim statements workspaces render.
The accessibility baseline (P5.12): semantic landmarks, a skip link whose
in-page anchor never re-routes the hash router (`src/router.ts`), labelled
interactive controls, and state conveyed in text, not colour alone.

## Verification gates (LISTED, NEVER EXECUTED)

```sh
npm install        # first install; re-verify the pins FIRST (below)
npm run dev        # vite dev server; proxies /api to the control plane
npm run build      # tsc -b && vite build -> static bundle in dist/
npm run typecheck  # tsc -b
npm run lint       # eslint .
npm run test       # vitest run
```

Requires Node 20+ (assumption — the exact engine range is confirmed at first
install). The dev proxy target is the control plane at
`http://127.0.0.1:8080` (`FLEXCORE_BIND_ADDR` default,
`apps/control-plane/src/config.rs`).

41 Vitest test files cover the tree (`src/**/*.test.ts(x)`): API contract
shapes, the live ordering/dedup gate, the router, the security surfaces, the
honesty/state vocabulary, and the twelve workspaces. All are **written, never
run** (headers say `UNRUN`/`WRITTEN-UNRUN`). Two recorded honest limitations
of the suite: it is renderToString/structural — it cannot run a real screen
reader or exercise keyboard focus (no DOM, no effects; the human's
browser-matrix and keyboard-only walkthrough remain the P5.12 gate), and the
hook/effect choreography of the live session is pinned structurally, not
executed.

## For operators (P5.29)

The operator guide is
`ledger/phase-05-operator-console-simulator/29_operator_guide.md`: what each
of the twelve workspaces shows, what every state means (the 14-state
lifecycle, the six quality states, the §12.6 solver outcomes — verbatim),
how to read a rejection and the audit trail, how to interpret live updates
(at-least-once: dedup, per-entity ordering, RESYNC), the role model, the
browser matrix, and — most importantly — what the console CANNOT do
(simulation-only; no field control; acknowledgement is not delivery; every
gap is stated, never filled). The guide describes this console as built;
the non-author review it requires is recorded there as OPEN (human gate).
`SUPPORT_MATRIX.md` records what each workspace targets per release.

## Version pins — SECONDARY SOURCES, RE-VERIFY AT FIRST INSTALL

All toolchain pins below come from dated third-party round-ups recorded in
`ledger/phase-05-operator-console-simulator/RESEARCH.md` (checked
2026-11-07). **They were never verified against a primary source.** Before the
first `npm install`, re-verify against the official release pages (react.dev/
versions, typescriptlang.org, vite.dev) and update `RESEARCH.md` and
`package.json` — do not silently upgrade (RESEARCH.md open task 1).

| Package | Pin | Source | Status |
|---|---|---|---|
| `react` / `react-dom` | `19.3.1` | secondary source | re-verify at first install |
| `typescript` | `~7.0.0` (devDep) | secondary source | re-verify at first install |
| `vite` | `^8.2.1` | secondary source | re-verify at first install |
| `@vitejs/plugin-react` | `^5.0.0` | secondary source | re-verify at first install |
| `@types/node` | `24.0.0` (devDep, EXACT) | recorded decision (below) — NOT named by RESEARCH.md | re-verify at first install |
| `vitest` | `^3.0.0` | **UNPINNED** — RESEARCH.md does not name a version | resolve at install, record |
| `eslint` | `^9.0.0` | UNPINNED | resolve at install, record |
| `typescript-eslint` | `^8.0.0` | UNPINNED; peer caps may lag the TypeScript major (RESEARCH.md risk note) | verify the combination at install |
| `@types/react` / `@types/react-dom` | `^19.0.0` | UNPINNED | resolve at install, record |

No lockfile exists and none may be committed from this loop. Licenses
(React MIT, Vite MIT, TypeScript Apache-2.0) are recorded as audit input for
the D4 gate, which is OPEN — the formal audit verifies at scaffold time.

### Recorded decision: `@types/node` 24.0.0 (exact pin)

The recorded verification gates run `tsc -b` (both `build` and `typecheck`),
and `tsconfig.app.json` includes all of `src/` — including the test files,
12 of which import `node:fs`/`node:path`/`node:url`. Those imports resolve to
type declarations shipped by `@types/node`, so it must be a DECLARED
dependency, not a transitive hope (FRV1-R3-001). RESEARCH.md does not name a
version for it; `24.0.0` is the well-known `@types/node` release matching the
Node 24 LTS era of the pinned `vite ^8.2.1` / `typescript ~7.0.0` toolchain.
It is pinned EXACT (unlike the UNPINNED caret entries) so the type surface the
gates compile against is recorded, not floating; re-verify against the
official `@types/node` release history (DefinitelyTyped) at first install.

## Recorded decisions

- **Hand-rolled hash router instead of TanStack Router** (DEFAULT): §7.5
  allows "TanStack Router and Query or the recorded equivalent", but
  RESEARCH.md verified no version for either; per the no-hallucinated-versions
  rule the console adds no unpinned routing dependency. Revisit trigger: the
  first workspace needing nested routes or deep links. TanStack Query is
  likewise deferred until a workspace's data layer needs it.
- **Fetch-based SSE instead of native `EventSource`**: the live endpoint
  requires the Authorization header; `EventSource` cannot set custom headers
  and a token in the URL query would be a credential leak. See
  `src/live/sseClient.ts`.
- **Generated OpenAPI client (P5.01 step 4) is deferred to the first
  install**: it needs the toolchain. Until then `src/api/types.ts` holds
  hand-written wire-shape mirrors with drift-note comments; treat a server
  DTO edit as a required edit there too.

## Limits (read before contributing)

- The console is a supervisory view. It performs no primary frequency
  response, no protection-grade action, and no direct device control — in any
  release (report §21.4; AGENTS.md).
- Acknowledgement is application-layer acceptance (in this phase: by the
  simulator), never physical delivery (§14.1).
- The live stream is at-least-once and unordered; client-side
  ordering/deduplication is implemented (P5.33) and consumers re-sync from
  the REST read APIs after a gap — never silently gap-fill.
- No demo data exists in this console; honest empty states are the default
  (§20.2).
- The login flow (`auth/oidcLogin.ts`) and every P13.18 management action are
  specified-not-implemented; the console renders that status honestly rather
  than a fabricated affordance.
