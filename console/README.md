# Neryva FlexCore — Operator Console (scaffold)

**Status: SCAFFOLD ONLY — nothing here has been installed, built, type-checked,
linted, or run.** This directory was created in the Phase 5 loop
(`ledger/phase-05-operator-console-simulator/`), whose rules ban installs,
builds, and test execution. Every file is complete source written against the
pinned toolchain; the first human install/build/test pass is the verification
point. See the decision record
`ledger/phase-05-operator-console-simulator/01_25_scaffold_live.md`.

## What this is

The operator console for the Neryva FlexCore platform: a **static bundle**
(Vite + React + TypeScript, report §7.5, §21.4) served by the control plane or
a CDN. It is a **view** of the simulation-only 0.x platform — it never issues
device commands, never defines authoritative permissions, and never displays a
simulated value as measured (§7.5, §20.2). Control authority in this phase is
Simulation / read-only (shadow) only (§21.4).

Currently present (P5.01 scaffold + P5.25 live updates):

- The app shell (`src/main.tsx`, `src/App.tsx`) with the control-authority
  banner, the claims-derived tenant indicator (§16.2), and the live-update
  status line.
- The hand-rolled hash router (`src/router.ts`) and the §20.1 workspace
  registry: **twelve honest "NOT IMPLEMENTED YET" stubs** (Overview,
  Portfolios, Sites, Assets, Grid, Schedules, Dispatch, Market & Programs,
  Simulation Lab, Plugins & Integrations, Audit & Reports, Administration).
  The real workspaces are Phase 5 waves 2–5.
- Typed API clients for the Phase 1–4 read surface (`src/api/`) mirroring the
  server DTOs (`apps/control-plane/src/dto*.rs`; contract tests in
  `apps/control-plane/tests/` pin the real shapes). There is **no write path
  and no execute-command client** — the server has no generic execute
  endpoint (§15.2) and the console adds none.
- The claims-derived tenant/session store (`src/state/tenantContext.ts`) —
  no tenant selector exists anywhere; the server derives tenancy from
  authenticated claims.
- The live-update client (`src/live/`): a fetch-based SSE consumer for
  `GET /api/v1/live` (tenant-scoped server-side), with reconnect backoff and
  the honest disconnect contract — "live updates paused — data may be stale",
  explicit re-sync from the REST read APIs, never silent gap-filling (§15.1).
- Vitest test files per skeleton area (`src/**/*.test.ts`) — **written, never
  run** (headers say `UNRUN`).

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

## Commands (LISTED, NEVER EXECUTED)

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
| `vitest` | `^3.0.0` | **UNPINNED** — RESEARCH.md does not name a version | resolve at install, record |
| `eslint` | `^9.0.0` | UNPINNED | resolve at install, record |
| `typescript-eslint` | `^8.0.0` | UNPINNED; peer caps may lag the TypeScript major (RESEARCH.md risk note) | verify the combination at install |
| `@types/react` / `@types/react-dom` | `^19.0.0` | UNPINNED | resolve at install, record |

No lockfile exists and none may be committed from this loop. Licenses
(React MIT, Vite MIT, TypeScript Apache-2.0) are recorded as audit input for
the D4 gate, which is OPEN — the formal audit verifies at scaffold time.

## Recorded decisions

- **Hand-rolled hash router instead of TanStack Router** (DEFAULT): §7.5
  allows "TanStack Router and Query or the recorded equivalent", but
  RESEARCH.md verified no version for either; per the no-hallucinated-versions
  rule the scaffold adds no unpinned routing dependency. Revisit trigger: the
  first workspace needing nested routes or deep links (waves 2a/2b).
  TanStack Query is likewise deferred until the data layer of wave 2.
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
- The live stream is at-least-once and unordered; ordering/dedup is P5.33.
  Consumers re-sync from the REST read APIs after a gap — never silently
  gap-fill.
- No demo data exists in this scaffold; empty/honest placeholders are the
  default (§20.2).
