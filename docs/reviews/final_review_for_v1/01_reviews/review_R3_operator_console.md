# Final Review R3 — Operator Console (Iteration 1)

**Reviewer:** Agent A (wave 2)
**Date:** 2026-11-17
**Status:** REVIEWED — static read only (no build/test executed)
**Finding totals:** P0: 1 · P1: 6 · P2: 4

## 1. Scope reviewed

Full census of `console/` (108 TypeScript sources + 9 config/doc files; LOC via `wc -l`).
Deep-read = every line read this review; structural skim = signatures, wiring points, call sites.

| Area | Files | LOC | Read depth |
|---|---|---|---|
| `console/src/api/` | 16 | 1,947 | Deep-read: all 12 client modules (`client`, `types`, `registry`, `telemetry`, `views`, `dispatch`, `flexibility`, `schedules`, `scenarioRuns`, `alarms`, `edgeStatus`, `plugins`, `research`) + `endpoints.test.ts`, `client.test.ts`, `alarms.test.ts` (skim). |
| `console/src/auth/` | 6 | 927 | Deep-read: `oidcLogin.ts`, `session.ts`, `stepUpFlow.ts` (WRITTEN-UNRUN by design — headers say so; reviewed for internal consistency, not runtime behaviour). |
| `console/src/security/` | 8 | 1,432 | Deep-read: `capabilityMatrix.ts`, `useCan.ts`, `modeControl.ts`, `denialLog.ts`. |
| `console/src/live/` | 5 | 1,409 | Deep-read: `types.ts`, `sseClient.ts`, `useLiveEvents.ts`, `ordering.ts`. |
| `console/src/state/` | 1 (+test) | 131 | Deep-read: `tenantContext.ts`. |
| `console/src/components/` | 22 | 3,623 | Deep-read: `honesty.tsx`, `ValueType.tsx`, `ConfirmHighImpact.tsx`, `tenantBanner.tsx`, `sessionBanner.tsx`, `stepUpApproval.tsx`, `modeGateNotice.tsx`, `alarmsPanel.tsx`, `availability.tsx`, `format.ts`, `stateVocabulary.ts`, `workspaceData.tsx`. Structural skim: the five `*.test.tsx` principle files. |
| `console/src/workspaces/` | 49 | 13,227 | Deep-read: `Dispatch.tsx` (the safety-critical approval flow), `SitesEdgeStatus.tsx` + `sitesEdgeStatus.ts`, `Overview.tsx` (head + loader), `registry.ts`, `adminManagement.ts`, `WorkspacePlaceholder.tsx`, `adminData.ts` (action-surface mirror), `pluginsData.ts` (head). Structural skim: `Grid`, `Schedules`, `AuditReports`, `SimulationLab`, `Sites`, `Assets`, `Portfolios`, `MarketPrograms`, `Administration`, `PluginsWorkspace`, `ResearchWorkspace`, `PluginsIntegrations`, `AdminManagementSection`, remaining `*Data.ts` (import/export + contract sweeps only). |
| root (`App.tsx`, `main.tsx`, `router.ts`) | 3 | 290 | Deep-read. |
| config/docs (`package.json`, `vite.config.ts`, `tsconfig*.json` ×3, `eslint.config.js`, `index.html`, `README.md`, `SUPPORT_MATRIX.md`) | 9 | — | Deep-read. |

Backend cross-check targets (grep-verified, not deep-read — R2 territory):
`apps/control-plane/src/{routes.rs,routes_phase2.rs,routes_phase3.rs,routes_phase4.rs,routes_alarms.rs,routes_edge.rs,routes_live.rs,routes_plugins.rs,routes_research.rs,dto*.rs,auth.rs,stepup.rs,main.rs}`,
`crates/flexcore-events/src/events.rs`, `crates/flexcore-authz/src/{action.rs,policy.rs,phase13_policies.rs}`,
`crates/flexcore-domain/src/{command_state.rs,command_lifecycle.rs}`, `crates/flexcore-contracts/src/alarm.rs`.

## 2. Method

1. **Backend route table first.** Extracted every `.route(` registration from
   `routes.rs`/`routes_phase2.rs`/`routes_phase3.rs`/`routes_phase4.rs`/`routes_alarms.rs`/`routes_edge.rs`/`routes_live.rs`/`routes_plugins.rs`/`routes_research.rs` (30 routes) into an endpoint-vs-route table, then walked each function of each `console/src/api/*.ts` module against it.
2. **DTO field-by-field mirror check.** Compared every interface in `api/types.ts`, `views.ts`,
   `alarms.ts`, `edgeStatus.ts`, `plugins.ts`, `research.ts`, `scenarioRuns.ts` against
   `dto.rs`, `dto_phase3.rs`, `dto_phase4.rs`, `dto_alarms.rs`, `dto_edge.rs`, `dto_plugins.rs`,
   `dto_research.rs` (wire names, optionality, envelope shapes).
3. **Query-parameter check.** Compared wire param names sent by the console (`asset`, `from`,
   `until`, `quantities`, `quality`, `cursor`, `limit`; `state`, `command_id`, `schedule_id`;
   `action`, `actor`, `object_type`, `object_id`, `from_event_unix_millis`, `to_event_unix_millis`;
   `device_id`, `severity`, `code`, `active_only`) against the serde param structs
   (`TelemetryQueryParams` routes_phase2.rs:256, `TelemetryHistoryParams`:1054,
   `Phase3ListParams` dto_phase3.rs:331, `Phase4ListParams` dto_phase4.rs:201,
   `AuditEventListParams` dto_phase4.rs:387, `AlarmListParams` routes_alarms.rs:97,
   `EdgeStatusParams` dto_edge.rs:74).
4. **SSE vocabulary check.** `live/types.ts` `FrozenEventTag` (26 tags) vs
   `events.rs::FROZEN_EVENT_NAMES` (26 PascalCase names, events.rs:496-523) and the
   SCREAMING_SNAKE rendering in `routes_live.rs` (`envelope.event_type`, :251/:369);
   `LiveEventEnvelope`/`LiveResyncNotice` field-for-field vs routes_live.rs:180/:224.
5. **Import/export sweep.** All 325 local `import … from` statements across 93 source files
   were machine-extracted; every imported name was checked against the real exports of the
   target module (including `export {}` lists and default exports); every relative specifier
   was resolved to an existing file.
6. **Vocabulary mirror checks.** `COMMAND_STATES`/`GATES`/`SOLVER_OUTCOMES`/`ROLL_TRIGGERS`/
   `QUALITY_STATES`/`FROZEN_EVENT_NAMES`/`DECLARED_OBJECTIVES` (stateVocabulary.ts) vs
   `command_state.rs`, `command_lifecycle.rs:169`, `dto_phase3.rs`, `events.rs`,
   `scheduling_objective.rs`; step-up denial/purpose vocabularies (auth/stepUpFlow.ts) vs
   `stepup.rs:68/:180`; `capabilityMatrix.ts` vs `action.rs` + `auth.rs` grant sets +
   `phase13_policies.rs`; alarm-ordering caption vs `dto_alarms.rs::alarm_page_envelope` sort.
7. **Safety-UI invariant sweep.** Control-authority mode handling (modeControl.ts,
   honesty.tsx), ConfirmHighImpact coverage, step-up flow honesty notes, acknowledgement≠delivery
   wording, tenant banner/guard, simulated labelling, grep for any endpoint the console calls
   that could constitute a write path.
8. **Config/claims check.** tsconfig path/include validity, vite proxy target vs
   `config.rs` bind address, `package.json` dependency set vs what the sources import,
   README/SUPPORT_MATRIX/index.html claims vs the actual feature state.

## 3. Checks performed, all consistent

1. **Endpoint paths + methods** — every console client function hits a route that exists with
   the same method: `GET /api/v1/tenants`(+`/{id}`, DELETE exists server-side, unused by console),
   `/portfolios`(+`/{id}`,+`/{id}/view`), `/sites`(+`/{id}`,+`/{id}/view`), `/assets`(+`/{id}`,
   +`/{id}/view`,+`/{id}/state`), `/telemetry/query`, `/telemetry/history`,
   `/flexibility/envelopes`, `/schedules`, `/optimization-runs`, `/dispatch-commands`
   (+`/{command_id}`), `/feasibility-checks`(+`/{check_id}`), `/audit-events`(+`/{event_id}`),
   `/alarms`, `/edge/status`, `/plugins`, `/research/status`, `/live`. **Clean.**
2. **Query-param wire names** — see Method 3; all match the serde structs verbatim, including
   the console's deliberate non-use of `as_of` (telemetry) and the snake_case filters.
   **Clean.**
3. **DTO mirrors** — every field of every mirrored interface matches the Rust DTO field name,
   type class, and optionality (checked `dto.rs` Phase 1/2 records + views,
   `dto_phase3.rs`, `dto_phase4.rs`, `dto_alarms.rs`, `dto_edge.rs`, `dto_plugins.rs`,
   `dto_research.rs`). **Clean.**
4. **SSE contract** — 26 tags exactly; `event_type` = frozen SCREAMING_SNAKE tag as the SSE
   `event:` name (routes_live.rs:369); envelope fields (`delivered_at`, `delivery`,
   `ordering_statement`, correlation fields) match; `RESYNC_REQUIRED` handled as a distinct
   SSE event name (routes_live.rs:301, sseClient.ts:105). **Clean.**
5. **Import/export sweep** — 0 missing module files, 0 imported names absent from the
   exporting module. **Clean.**
6. **Lifecycle/quality/event vocabularies** — 14 command states in frozen order
   (command_state.rs:328-331+), 3 gates (command_lifecycle.rs:169-171), 6 quality states,
   26 event names, solver outcomes, roll triggers, declared objective — all byte-consistent
   with the crates. **Clean.**
7. **Step-up vocabularies** — 9 `StepUpDenialKind`s and 2 purpose kinds mirror
   `stepup.rs::StepUpDenial`/`StepUpPurpose` exactly; unknown-denial renders fail-closed.
   **Clean.**
8. **Alarm display claim** — `alarmsPanel.tsx` caption "Ordered by severity rank then alarm
   id" matches the server sort (`dto_alarms.rs:144-149`, rank desc then id). **Clean.**
9. **Scenario-run display contract** — `api/scenarioRuns.ts` targets `/api/v1/scenarios/runs`,
   which has no server route; the module header, the workspace, and the tests record this as
   the display contract with an honest error state. **Clean (by design).**
10. **Router coverage** — all 12 registry ids resolve to real components in `App.tsx`; an
    unknown hash falls back to Overview (the 404-equivalent). One interaction defect found
    (skip link, FRV1-R3-005).
11. **Write-path absence** — grep over all `api/*.ts`: only `GET` is ever issued; `createApiClient`
    exposes `get`/`request` only and no module constructs a POST/PUT/DELETE/PATCH. No execute,
    transition, acknowledge, enable, promote, or activate endpoint is called or invented.
    **Clean.**
12. **Safety-UI invariants** — acknowledgement≠delivery wording central in `honesty.tsx`;
    simulated labelling via `SIMULATED_STATE_MEANINGS` + `CommandStateBadge` (single source,
    enforced by `honestyStructural.test.ts`); mode is a deployment constant never read from
    user input (`modeControl.ts:82`); step-up panel states LOCAL-UI-EVENT honestly and the
    enforced-protocol machine (`auth/stepUpFlow.ts`) can never reach `verified` without a
    server verdict; ConfirmHighImpact covers the one console-triggerable high-impact action
    (approval) and is used by `Dispatch.tsx:509`; tenant banner claims-derived with no
    selector anywhere (structural tests pin it). **Clean.**
13. **`tenantActionGuard`/`TenantMismatchBadge` unused in record tables** — recorded BY
    DESIGN in `tenantBanner.tsx:36-41` (decision record 32_33_tenant_live_ordering.md owns
    the wiring point); not counted as a defect.
14. **`seedAnchor`/`DemoDataBadge` defined-for-use** — recorded as unwired hooks in their
    module headers; not counted as defects.
15. **tsconfig/vite** — no path aliases to validate; `include: ["src"]` matches the tree;
    vite proxy target `http://127.0.0.1:8080` matches the control-plane default bind.
    **Clean** (but see FRV1-R3-001 for the type-coverage gap this config produces).

## 4. Findings

### FRV1-R3-001 [P0] [phase-05] `@types/node` is missing while 12 test files under `src/` import `node:fs`/`node:path`/`node:url` — `npm run build` and `npm run typecheck` cannot compile

- Status: OPEN
- Location: `console/package.json:19-28` (devDependencies: `@types/react`, `@types/react-dom`,
  `@vitejs/plugin-react`, `eslint`, `typescript`, `typescript-eslint`, `vite`, `vitest` — no
  `@types/node`); `console/tsconfig.app.json:32` (`"include": ["src"]` — the app project
  compiles the test files); e.g. `console/src/auth/oidcLogin.test.ts:5-7`,
  `console/src/auth/stepUpFlow.test.ts:7-9`, `console/src/components/honestyStructural.test.ts:33-35`,
  `console/src/components/sessionBanner.test.tsx:5-7`, `console/src/components/stepUpApproval.test.tsx:13-15`
  — 12 files total import `node:fs` / `node:path` / `node:url`.
- Issue: `tsc -b` (both the `build` and `typecheck` scripts) compiles every file under `src/`,
  including the structural tests. `import … from 'node:fs'` resolves to type declarations
  shipped by `@types/node`, which is not in the declared dependency set and there is no lockfile
  to supply it transitively. With `moduleResolution: "bundler"` and no `types` override, these
  imports are TS2307 errors: the first human install/typecheck pass — the recorded verification
  gate for this whole scaffold — fails before any console code is exercised. This is a
  would-not-compile-class defect, exactly the class this review is mandated to catch, and it is
  provable statically.
- Recommendation: add `@types/node` to `devDependencies` (version resolved and recorded at
  first install, per the x-pin-policy; or a `devDependencies` comment marking it UNPINNED like
  the other unresolved toolchain entries). Alternatively exclude `**/*.test.*` from
  `tsconfig.app.json` and add a third tsconfig project for tests — but the single-dependency fix
  is smaller and matches the template convention.

### FRV1-R3-002 [P1] [phase-05] Console alarms surfaces permanently render the error state under the shipped control-plane composition (console-side consequence of FRV1-R2-001)

- Status: OPEN (root cause already recorded as **FRV1-R2-001** [P1] in `review_R2_control_plane_lifecycle_authz_audit.md` — no duplicate server-side finding is raised here)
- Location: console: `console/src/api/alarms.ts:106-125` (`listAlarms`),
  `console/src/workspaces/Overview.tsx:97-99` + `console/src/components/alarmsPanel.tsx:85-97`;
  backend: `apps/control-plane/src/main.rs:22,144` (`phase4_default_policies()`),
  `apps/control-plane/src/auth.rs:500-521` (`phase5_default_policies` is the only grant source
  for `alarm.read` and has no production caller), `apps/control-plane/src/routes_alarms.rs:163`
  (`authorize_tenant_scoped(…, Action::AlarmRead)`).
- Issue: the shipped composition root grants phases 1–4 only; `GET /api/v1/alarms` authorizes
  `Action::AlarmRead`, which only `phase5_default_policies()` grants. Every authenticated call
  returns 403, so the Overview "Key alarms" panel and the per-asset alarm panels can never
  display data. The console behaves CORRECTLY (it renders the verbatim 403 error block, never a
  fake empty state — the honesty contract holds); the finding is recorded here so the release
  checklist knows the console's §20.1 item-1 "key alarms" element is dark until R2-001 is fixed.
- Recommendation: fix at the composition root (R2-001's recommendation: compose
  `phase5_default_policies()`); after that fix, the console needs no change. Until then, do not
  ship release notes implying the Overview alarms panel is functional.

### FRV1-R3-003 [P1] [phase-13] The console carries two mutually inconsistent mirrors of the server authorization surface, and `canAction`'s denial reason asserts a falsehood about the API for the `site-approver` role

- Status: OPEN
- Location: `console/src/security/capabilityMatrix.ts:39-44` (`ROLES` = "the closed
  four-role vocabulary"), `:294-351` (`canAction` — the role-outside-vocabulary branch returns
  "…the API would reject it — deny"); `console/src/workspaces/adminManagement.ts:59-65`
  (`PHASE13_ROLES` = five roles including `site-approver`) and `:75-107`
  (`PHASE13_GRANT_TABLE` — `command.approve` → `site-approver` at site/asset scope);
  backend: `crates/flexcore-authz/src/policy.rs:69,80` (`Role::SiteApprover`),
  `action.rs:158-168,248-299` (`apikey.*` actions exist on the enum),
  `phase13_policies.rs:47-100` (recorded grants), `apps/control-plane/src/auth.rs:107-115`
  (`parse_role` already maps `site-approver`); cross-ref FRV1-R2-009 (phase13 policies composed
  nowhere in the shipped root).
- Issue: the P13.05 extension added a fifth role and an `apikey.*` action family to the closed
  server vocabularies. `capabilityMatrix.ts` still declares four roles and no `apikey.*`/market.*
  entries, while `adminManagement.ts` (same console, same release) mirrors the five-role
  vocabulary and the `apikey.*` grants. Consequences: (a) a `site-approver` dev session — which
  `auth.rs::parse_role` already accepts server-side — is denied every console action with the
  reason "role 'site-approver' is outside the closed four-role vocabulary … the API would reject
  it". Under the recorded phase-13 composition that API claim is FALSE (the API grants
  `command.approve` to site-approver at site/asset scope); today it is true only because the
  phase-13 grant set is composed nowhere (R2-009). Either way the two console mirrors disagree,
  and the Dispatch approval gate (`Dispatch.tsx:85,415`) would deny a role the platform's
  recorded design explicitly grants. (b) The matrix header itself says "THE ENUM IS
  AUTHORITATIVE; the mirror can drift until hand-updated" — this is exactly that drift, now
  visible *within one console*.
- Recommendation: extend `capabilityMatrix.ts` — add `site-approver` to `ROLES` with the
  single `command.approve` site/asset entry, add the `apikey.*` entries (or record them as
  deliberately-unmirrored with a drift note pointing at `adminManagement.ts`), and soften the
  outside-vocabulary denial reason ("the API would reject it" → "the recorded mirror knows no
  such role; the API decides independently"). Pin the five-role vocabulary in
  `capabilityMatrix.test.ts` so both mirrors agree.

### FRV1-R3-004 [P1] [phase-06] The P6.25 edge-status panel is unreachable: no route renders `SitesEdgeStatusSection`, contradicting the module header's claim that the Sites workspace embeds it

- Status: OPEN
- Location: `console/src/workspaces/SitesEdgeStatus.tsx:61` (`SitesEdgeStatusSection` — its
  only non-test reference is its own definition), `console/src/workspaces/sitesEdgeStatus.ts:2-6`
  ("The Sites workspace embeds this extension as a per-edge status panel"),
  `console/src/workspaces/Sites.tsx:20-35` (imports: views, dispatch, ValueType, format — no
  SitesEdgeStatus, no `loadEdgeStatuses`), `console/src/api/edgeStatus.ts:90-99`
  (`fetchEdgeStatus` — called only by `sitesEdgeStatus.ts`), `console/src/App.tsx:61-76`
  (composes Plugins+Inventory, SimulationLab+Research, Administration+Management — no edge
  section anywhere).
- Issue: the complete P6.25 feature — data loader, mapping, panel, confirmation affordance,
  and its 298-line test file — exists but is composed into nothing. `App.tsx` demonstrably
  follows the additive-panel pattern for the P7.25 and P8.27 waves (`PluginsIntegrationsWithInventory`,
  `SimulationLabWithResearch`) but never applies it to the Sites route. The module header's
  embedding claim is therefore false as shipped, and the `GET /api/v1/edge/status` client is
  dead code. (The server-side seam gap — `edge_enrollment` never composed, the route answers
  infrastructure-error — is separately recorded as FRV1-R2-002; fixing only that would still
  leave this panel invisible.)
- Recommendation: wire it the recorded way — render `<SitesEdgeStatusSection data={…} />`
  below `SitesWorkspace` (additive composition like P7.25/P8.27), fed by
  `loadEdgeStatuses(sites' edge identities, client)`; or, if deferral is intended, correct the
  `sitesEdgeStatus.ts:2-6` header to state that the panel is composed by a later step, so the
  code does not claim an integration it does not have.

### FRV1-R3-005 [P1] [phase-05] The accessibility skip link re-routes the app: `href="#workspace-main"` collides with the hash router and resets the workspace to Overview

- Status: OPEN
- Location: `console/src/App.tsx:136-138` (`<a className="skip-link" href="#workspace-main"
  data-testid="skip-link">`), `:178` (`<main id="workspace-main" tabIndex={-1}>`);
  `console/src/router.ts:20-24` (`parseRoute` — any hash that is not a workspace id returns
  `DEFAULT_ROUTE`, `:17` = `'overview'`).
- Issue: the skip link is the first focusable element and uses a bare-fragment hash. Clicking
  it (the keyboard-only path it exists for) sets `location.hash = "#workspace-main"`; the
  `hashchange` listener re-runs `parseRoute`, which finds no workspace named
  `workspace-main` and silently navigates to the **Overview** workspace. A keyboard user on
  Dispatch, Audit & Reports, or Administration who uses the skip link is teleported to a
  different workspace (losing filter/detail state), and the focus/scroll benefit is destroyed
  by the re-render. Two URL schemes (routes and in-page anchors) share one hash channel.
- Recommendation: make the skip link a button that calls `focus()` on the main element (keep
  `tabIndex={-1}`), or keep the anchor but `preventDefault` + `main.focus()` in an onClick so
  the hash never changes; alternatively namespace route hashes (`#/w/overview`) so in-page
  fragments never parse as routes. Smallest fix: the button/onClick form.

### FRV1-R3-006 [P1] [phase-05] The live re-sync contract is wired to nothing: no code performs the re-sync the shell text claims, and no workspace ever consumes a live event

- Status: OPEN
- Location: `console/src/App.tsx:154` (shell consumes only `live.status` +
  `live.lastDeliveredAt`), `:211-216` (paused-stale line: "The console re-syncs explicitly on
  reconnect; no gap is filled silently (P5.25)"); `console/src/live/useLiveEvents.ts:49,60,161-165`
  (`needsResync`, `lastAppliedEvent`, `acknowledgeResync` — exported, never consumed outside
  `live/`); `console/src/live/ordering.ts:351-366` (`seedAnchor` — header itself: "no
  workspace wires it yet"); `console/src/components/workspaceData.tsx:43-62`
  (`useWorkspaceData` — mount-only effect, no re-fetch trigger);
  `console/src/components/honesty.tsx:347-353` (`LIVE_DISPATCH_LABEL_NOTE` — "Live dispatch
  events … update the same lifecycle badge", rendered in Dispatch.tsx:697-699).
- Issue: grep confirms **no** consumer of `needsResync`, `acknowledgeResync`,
  `lastAppliedEvent`, or `lastEvent` outside `live/` itself and tests. Workspaces fetch once on
  mount and never re-fetch; after any disconnect or `RESYNC_REQUIRED`, the shell shows the
  paused-stale line while every workspace keeps displaying its pre-gap data indefinitely —
  and the shell's sentence "The console re-syncs explicitly on reconnect" describes behaviour
  no code performs. Likewise `LIVE_DISPATCH_LABEL_NOTE` tells the operator live events update
  the lifecycle badge, but no workspace applies live events to any displayed record. The
  underlying client machinery (backoff, dedup, ordering, resync flagging) is complete and
  honest; the last wiring step — consumers re-fetching on `needsResync` and acknowledging —
  is missing, and two rendered honesty notes over-claim as a result.
- Recommendation: either (a) wire the recorded contract: pass `needsResync` into the data
  workspaces (or a shell-level callback) that re-invokes their loaders and then calls
  `acknowledgeResync()`, seeding anchors via `seedAnchor`; or (b) if that composition is
  deferred, correct the two rendered statements to say the re-sync/apply step is recorded as
  the next integration point (the P5.25-style honesty used everywhere else in this console) —
  the console must not display a promise of behaviour it does not contain.

### FRV1-R3-007 [P1] [phase-05] `console/README.md` describes the P5.01 scaffold state, not the shipped console: stub claims, a stale P5.33 claim, and a stale wave framing

- Status: OPEN
- Location: `console/README.md:26-29` ("twelve honest 'NOT IMPLEMENTED YET' stubs … The real
  workspaces are Phase 5 waves 2–5"), `:42-43` ("Vitest test files per skeleton area"),
  `:122` ("ordering/dedup is P5.33" — listed under "Limits" as not implemented);
  actual state: `console/src/workspaces/registry.ts:4-9` (P5.02–P5.04 implemented),
  real components for all twelve workspaces (`App.tsx:61-76`), ordering/dedup implemented
  (`console/src/live/ordering.ts`, header "P5.33 — client-side ordering and deduplication"),
  plus P6.25/P7.25/P8.27/P13.17/P13.18 panels the README never mentions.
- Issue: the README is the human-facing summary of this directory; as written it tells an
  operator/reviewer that every workspace is a placeholder and that ordering/dedup does not
  exist — both false, and the second one contradicts the code it sits next to. It also omits
  every post-P5 feature (edge status, plugin inventory, research status, session banner,
  step-up flow, admin catalog), so the README undersells and misstates the release content.
  SUPPORT_MATRIX.md stays accurate (it only claims targets, never support) — only README drifts.
- Recommendation: update README's "Currently present" list to the twelve implemented
  workspaces + the additive panels, drop "NOT IMPLEMENTED YET" stub wording, and restate the
  live-stream limit as "at-least-once; client-side dedup/ordering implemented (P5.33);
  consumers re-sync from the REST APIs after a gap" — or reference FRV1-R3-006's outcome
  once that is decided.

### FRV1-R3-008 [P2] [phase-05] `api/endpoints.test.ts` is tautological: the endpoint modules it names are voided, the assertions are literal strings, and its header claims a drift-detection property it does not have

- Status: OPEN
- Location: `console/src/api/endpoints.test.ts:44-57` (asserts `'/api/v1/portfolios'`
  matches `/^\/api\/v1\/portfolios$/` etc., with `void listPortfolios; void listSites; …`),
  `:60-68` (drives `client.get('/api/v1/telemetry/query?…')` directly, then `void queryTelemetry;`),
  `:70-90` (asserts a hardcoded array `toHaveLength(6)`, voids all six module imports);
  header `:1-2` and inline note `:40-46` ("a server rename breaks this test on purpose
  (contract drift fails)").
- Issue: the test never calls any endpoint-module function, so a path change inside
  `registry.ts`/`dispatch.ts`/etc. cannot fail it — the exact drift the header promises to
  catch. Every assertion compares a literal to its own regex; the test cannot fail except by
  deletion. It gives false confidence that the console↔route table is pinned. (This review's
  manual sweep, recorded in §3.1, confirms the real paths match today.)
- Recommendation: make the tests exercise the modules with a recording client
  (`createApiClient({ fetchImpl })` capturing the URL) and assert the produced paths — the
  pattern already used in `client.test.ts` and by `alarms.test.ts`; drop the `void` bindings
  and the misleading note.

### FRV1-R3-009 [P2] [phase-05] Stub-era comment drift: `index.html` and `registry.ts` still describe twelve NOT-IMPLEMENTED-YET stubs

- Status: OPEN
- Location: `console/index.html:11-12` ("The visible surface is the app shell and twelve
  'NOT IMPLEMENTED YET' workspace stubs; the real workspaces are Phase 5 waves 2–5");
  `console/src/workspaces/registry.ts:10-13` ("STILL STUBS (waves 2b+): schedules, dispatch,
  market-programs, simulation-lab, plugins-integrations, audit-reports, administration — each
  renders the honest 'NOT IMPLEMENTED YET' placeholder").
- Issue: every named workspace has a real implementation (Schedules.tsx 589 lines,
  Dispatch.tsx 775, etc.); `WorkspacePlaceholder.tsx` has zero remaining consumers (grep: only
  its own file and the registry comment reference it). The comments contradict the files
  directly below them — comment drift on the two files a new contributor reads first.
  (`WorkspacePlaceholder.tsx` itself is now dead code; harmless, tree-shaken, but the comment
  should not point at it as a live pattern.)
- Recommendation: update both comments to the implemented state (and optionally delete
  `WorkspacePlaceholder.tsx` or mark it retained-for-future-stubs with one sentence).

### FRV1-R3-010 [P2] [phase-05] `ModeGateNotice` and `useCanInMode` have no consumer: the P5.27 mode-gate statement exists in code but is never rendered in any workspace or the shell

- Status: OPEN
- Location: `console/src/components/modeGateNotice.tsx:37-67` (`ModeGateNotice` — only
  non-test reference is its own definition); `console/src/security/modeControl.ts:291-304`
  (`useCanInMode` — only the test calls it; `Dispatch.tsx` composes `useCan` +
  `gateActionForMode` + `combineChecks` manually instead, which is functionally equivalent).
- Issue: the additive-component pattern (headers say "rendered beside the P5.13 indicator
  where a workspace needs the gate statement") was never applied: no workspace renders the
  notice, so the deployment/mode-gate explanation (MODE_ENFORCEMENT_NOTE, mixed-mode note,
  non-exercised-modes note) never reaches a user outside tests. The mode gate still *runs*
  (Dispatch's approval flow), so this is display loss, not an enforcement gap — but the P5.27
  step-3 deliverable is effectively invisible.
- Recommendation: render `<ModeGateNotice />` in the app shell header next to
  `<ControlAuthorityModeIndicator />` (one additive edit, matching the P7.25/P13.17
  composition precedent), or fold its notes into the indicator; and either route
  `Dispatch`'s gate through `useCanInMode` or mark it a thin wrapper for tests.

### FRV1-R3-011 [P2] [phase-05] The local denial log records capability denials and step-up failures that no UI surface ever shows

- Status: OPEN
- Location: `console/src/security/denialLog.ts:46-52` (`getConsoleDenials`/`clearConsoleDenials`
  — no non-test callers); producers: `console/src/components/stepUpApproval.tsx:140-151`
  (`recordStepUpDenial` ← Dispatch.tsx:438-455) and capability denials described at
  `console/src/workspaces/Dispatch.tsx:480-482` ("this denial is recorded locally only
  (security/denialLog.ts)"). Compare: the sibling local logs DO have display surfaces —
  `PluginsWorkspace.tsx:413` and `ResearchWorkspace.tsx:384` render their request counts.
- Issue: the module's stated purpose ("so the operator/console diagnostics can explain what
  the user saw", denialLog.ts:9-12) is unfulfilled: entries are written into an in-memory ring
  buffer that nothing renders, so an operator who triggers a step-up failure or sees a
  capability denial has no way to review the recorded events; only tests read them. Small
  gap, but it makes two rendered statements ("recorded locally") point at an invisible log.
- Recommendation: render a small diagnostics block (count + last entry) in the Administration
  workspace or the session banner area, or scope the module header down to "test-observable
  diagnostics" so the claim matches the reality.

## 5. Release-readiness verdict for this area

The console is architecturally strong for a first public release: the API layer is a faithful,
verified mirror of the control-plane surface (paths, methods, query params, and every DTO
field checked — zero contract drift in the data plane), the 26-event SSE vocabulary matches the
frozen crate exactly, the safety-UI invariants hold (mode is deployment-configuration only,
the three gates are displayed separately, acknowledgement is never delivery, simulated
labelling is centralised and structurally pinned, the write path is provably absent, and every
unimplemented capability is an explicit honest gap rather than a plausible filler). The one
P0 is mechanical but blocks the recorded verification gate: `@types/node` is missing while the
compile gate includes the node-builtins-using tests. Before release, the must-fix set is
FRV1-R3-001 plus the honesty-critical FRV1-R3-005/006/007 (the skip link that re-routes, the
re-sync claim no code performs, and the stale README) — none is large, but all three put
statements on screen or in docs that the code contradicts. FRV1-R3-002/003/004 need
coordination with the control-plane fixes already recorded in R2 (alarms composition, edge
seam, phase-13 composition) and a decision on whether the edge panel ships or its header
changes. The P2 set (tautological endpoint test, comment drift, unwired ModeGateNotice,
invisible denial log) is optional polish that nonetheless cheaply improves the console's
central asset: its claim-level honesty.
