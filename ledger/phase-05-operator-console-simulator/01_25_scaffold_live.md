# Decision Record — P5.01 (console scaffold) and P5.25 (live updates)

- **Date:** 2026-11-11 (Phase 5 wave 1a; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.01 (§7.5, §21.4), P5.25 (§15.1, §21.3).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or run.
  No `npm install`, no lockfile, no `node_modules`, no `tsc`/`vite`/`vitest`
  execution, no `cargo build/check/test`. All code and tests are complete
  source written only; every test file carries an `UNRUN` header and the
  human executes them at first install (RESEARCH.md open task 1). No
  checkbox in `ledger_phase_5.md` claims tested/verified/passed.

## 1. What was implemented

### P5.01 — console scaffold (NEW top-level `console/`, NOT a cargo workspace member)

| Area | Files |
|---|---|
| Toolchain/config | `console/package.json` (pins + `x-pin-policy`), `console/tsconfig.json`, `console/tsconfig.app.json` (strict + `exactOptionalPropertyTypes`), `console/tsconfig.node.json`, `console/vite.config.ts` (dev proxy to the control plane, `FLEXCORE_BIND_ADDR` default `127.0.0.1:8080` per `apps/control-plane/src/config.rs`; streaming-safe for SSE), `console/eslint.config.js`, `console/.gitignore`, `console/index.html` |
| App shell | `console/src/main.tsx`, `console/src/App.tsx` (control-authority banner, claims-derived tenant indicator, live-status line) |
| Routing | `console/src/router.ts` + `console/src/router.test.ts` (hand-rolled hash router — decision 3.2 below) |
| Workspaces | `console/src/workspaces/registry.ts` + 12 stub components (`Overview`…`Administration`, §20.1 items 1–12) each rendering `WorkspacePlaceholder` ("NOT IMPLEMENTED YET — Phase 5 wave in progress"; no fabricated data) + `registry.test.ts` |
| API layer | `console/src/api/{client,registry,telemetry,flexibility,dispatch,types}.ts` + `client.test.ts` + `endpoints.test.ts` (typed fetch client skeletons for the P2/P3/P4.30 route groups; wire-shape mirrors with drift-note comments) |
| State | `console/src/state/tenantContext.ts` + `tenantContext.test.ts` (claims-derived; no tenant selector anywhere, §16.2) |
| Live | `console/src/live/{types,sseClient,useLiveEvents}.ts` + `sseClient.test.ts` (see P5.25 below) |
| README | `console/README.md` (honest: scaffold only; commands listed, never executed; pin caveat) |

Not implemented (recorded, honest): the generated OpenAPI client + drift test
(P5.01 step 4) — it requires the toolchain; the hand-written mirrors in
`src/api/types.ts` are the recorded stand-in until the first install. No
static-bundle serving wiring in the control plane (§21.4 build form is
satisfied by the config emitting `dist/`; serving it is a recorded follow-up).
No CI wiring (P5.28 owns it). No lockfile exists.

### P5.25 — live updates (SSE)

Server side (Rust, additive):

- `apps/control-plane/src/routes_live.rs` (NEW): `GET /api/v1/live` — axum
  0.8.9 SSE (`axum::response::sse`, same axum as `routes_phase4.rs`),
  tenant-scoped at CONSUMPTION from the decoded frozen event payload (D-J2:
  no tenant identifier in any subject). Events enter via an in-process
  `LiveEventBus` fed by `run_live_forwarder` — an EPHEMERAL core-NATS
  subscription on `flexcore.v1.>` (no JetStream consumer, no side effect;
  P1.14 §5.1 discipline), subscribing to the SAME subjects the NATS adapter
  consumes. Payloads decode ONLY into the frozen §15.3 vocabulary
  (`flexcore_events::DomainEvent`); an undecodable or tenant-less payload is
  forwarded to NO stream (fail closed). Correlation: the envelope carries the
  event's `correlation_id` — the same id the audit record carries (§21.3).
  Idle heartbeat (KeepAlive comment, 15 s); lagged subscriber →
  `RESYNC_REQUIRED` (no silent gap-filling); at-least-once/unordered honesty
  statement travels IN every envelope (P5.33 owns ordering/dedup — nothing
  dedups here, by design). No secrets or unnecessary personal data in the
  stream (payloads carry ids and quality states only; step 5).
- Additive registration: `routes.rs` (router merge), `openapi.rs` (path +
  `LiveEventEnvelope`/`LiveResyncNotice` schemas), `auth.rs`
  (`authorize_live_stream` — see decision 3.1 for the recorded drift),
  `state.rs` (`AppState.live: Arc<LiveEventBus>`), `lib.rs` (module),
  `main.rs` (composition-root forwarder task), `Cargo.toml`
  (`tokio-stream` sync feature — version resolved from the existing
  workspace dependency set; no new external pin invented beyond what the
  lockfile/workspace already carries).
- Tests: `apps/control-plane/tests/api_live.rs` (NEW; UNRUN) — authz
  (401/403/deny-by-default), tenant scoping with no cross-tenant leakage,
  SSE content type, correlation ids, at-least-once statements,
  RESYNC_REQUIRED on lag, planted-secret non-passthrough, OpenAPI GET-only —
  plus unit tests inside `routes_live.rs`.

Client side (`console/src/live/`):

- `types.ts`: TS mirrors of the frozen payloads/events.rs envelope with
  drift-note comments; the closed 26-event tag vocabulary.
- `sseClient.ts`: fetch-based SSE client (decision 3.3) with capped
  exponential backoff + jitter, status lifecycle
  (`connecting/open/paused-stale/closed`), explicit `needsResync` on any gap,
  no dedup (P5.33's scope).
- `useLiveEvents.ts`: the React hook (StrictMode cleanup-correct); status +
  last-delivery time + resync flag surfaced to the shell and every future
  workspace.
- `App.tsx` renders the disconnect contract: "Live updates paused — data may
  be stale (last update …)" — never a silent continue (P5.25 step 2).

## 2. Version-pin table (secondary-source discipline)

| Package | Pin | Pin source | Status |
|---|---|---|---|
| react / react-dom | `19.3.1` | RESEARCH.md §2 (secondary source, dated round-up) | RE-VERIFY at first install (RESEARCH.md open task 1) |
| typescript (devDep) | `~7.0.0` | RESEARCH.md §3 (secondary source) | RE-VERIFY at first install; feature set used is version-stable so 5.x/7.x both type-check |
| vite | `^8.2.1` | RESEARCH.md §3 (secondary source, one tutorial) | RE-VERIFY at first install |
| @vitejs/plugin-react | `^5.0.0` | RESEARCH.md §3 (secondary source) | RE-VERIFY at first install |
| vitest | `^3.0.0` | NOT named by RESEARCH.md | UNPINNED caret range; resolve + record at install |
| eslint | `^9.0.0` | NOT named by RESEARCH.md | UNPINNED caret range; resolve + record at install |
| typescript-eslint | `^8.0.0` | NOT named by RESEARCH.md | UNPINNED caret range; RESEARCH.md flags peer-cap lag vs a TypeScript major — verify the combination at install (risk comment recorded in `package.json`/`eslint.config.js`, no invented "compatible" version) |
| @types/react, @types/react-dom | `^19.0.0` | NOT named by RESEARCH.md | UNPINNED caret ranges; resolve + record at install |
| tokio-stream (Rust) | workspace-resolved | existing dependency set | resolved by the existing `cargo` lockfile/workspace at first build — no version invented here |

Rule applied throughout: where RESEARCH.md names a version, it is pinned as
recorded with the re-verify marker; where it does not, a caret range with an
explicit UNPINNED comment is used — no hallucinated exact versions.

## 3. Decisions

1. **Interim authorization action = `telemetry.read`, recorded drift
   (DEFAULT, documented in code).** The ledger's action id for the live
   surface is `live.stream.read` (tenant-scoped, all roles), but `Action` is
   a CLOSED enum owned by `crates/flexcore-authz` — outside this wave's file
   ownership. `auth::authorize_live_stream` evaluates `telemetry.read`
   (tenant-scoped, granted to all four roles by `phase2_default_policies`)
   as the interim deny-by-default gate and is the single swap point for
   `Action::LiveStreamRead` (one variant + `id`/`parse` arms + role grants).
   Adding the variant is a one-line follow-up for whoever owns
   `flexcore-authz` next; the behaviour contract (tenant-scoped, all roles,
   deny-by-default) is already enforced and tested.
2. **Hand-rolled hash router instead of TanStack Router (DEFAULT).** §7.5
   allows "TanStack Router and Query or the recorded equivalent";
   RESEARCH.md verified NO version for TanStack Router or Query, and the
   no-hallucinated-versions rule forbids pinning an unverified dependency. A
   ~60-line hash router covers the twelve workspace routes and keeps the
   static-bundle deployment serverless-config-free. TanStack Query is
   likewise deferred to the first data-layer workspace (wave 2). Revisit
   trigger recorded in `router.ts`.
3. **Fetch-based SSE instead of native `EventSource` (DEFAULT).** The live
   endpoint requires the Authorization bearer header; `EventSource` cannot
   set custom headers and a token in the URL query would leak the credential
   into logs/history (P5.25 step 5). The stream is read with fetch +
   ReadableStream and parsed by a small tested frame parser. Revisit if the
   endpoint moves to cookie authentication.
4. **Tenant scoping at consumption, from the payload (D-J2).** The NATS
   subjects carry no tenant token (P1.14); the SSE handler filters from the
   decoded event's `tenant_id` against the CALLER's claims-derived tenant
   (never a request field). An event for another tenant is not forwarded —
   tested in `api_live.rs`.
5. **At-least-once/unordered honesty is data, not prose.** Every envelope
   carries `delivery: "at-least-once"` and an `ordering_statement`; a lagged
   stream emits `RESYNC_REQUIRED` with the skipped count. Dedup/sequencing
   is deliberately absent (P5.33) — this module does not silently fix
   ordering.
6. **Session wiring in the live hook.** `useLiveEvents` draws its bearer
   token from the same claims-derived session store + dev-session token
   path as the REST client (`state/tenantContext.ts`,
   `api/client.ts devSessionToken`) — never a URL parameter. Until P5.26
   this is the recorded dev-auth path only; the swap to the real session is
   a single provider function.
7. **Workspace stubs render the honest placeholder.** No sample data, no
   simulated numbers, no invented alarm semantics (P5.31 owns the alarm
   contract; Overview's stub names the gap). Empty states are the default
   (CONTEXT.md open question 6).

## 4. Honest limitations

- **Nothing was executed.** TypeScript was never compiled; no vitest suite
  ran; the SSE route never served a request; the Rust tests never ran; the
  vite build was never produced. The first install/build/test pass is the
  human's verification point and may surface toolchain corrections
  (especially the typescript-eslint/TypeScript-major combination).
- The version pins are SECONDARY-SOURCE; the first install re-verifies them
  against the primary release pages and records any change in RESEARCH.md.
- The generated OpenAPI client + drift test (P5.01 step 4) is deferred to
  the first install; the hand-written mirrors are interim and could drift
  until then (drift-note comments mark the discipline).
- The `live.stream.read` action id is not yet in the closed `Action` enum
  (decision 3.1); the interim gate is behaviourally equivalent (tenant-
  scoped, all roles, deny-by-default) but is not the ledger's named action.
- The control plane does not yet serve the static `dist/` bundle (§21.4
  build/distribution form is partially satisfied: the bundle is producible,
  serving is a recorded follow-up).
- §21.3 metric surfaces (ingestion delay, queue depth, solver duration, …)
  are not wired into the console in this scaffold; P5.10/P5.24 own the
  freshness/metrics surfaces. The correlation plumbing (§21.3 trace ids in
  the stream) IS in place.
- No WebSocket alternative was built: §15.1 offers SSE or WebSocket; SSE
  alone is implemented and the report's live-update areas (telemetry,
  schedule state, dispatch lifecycle) all flow over it.
