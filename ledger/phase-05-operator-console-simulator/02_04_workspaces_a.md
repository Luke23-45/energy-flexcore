# Decision Record — P5.02 + P5.03 + P5.04 (Overview, Portfolios/Sites/Assets, Grid workspaces)

- **Date:** 2026-11-11 (Phase 5 wave 2a; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.02 (§20.1 item 1, §20.2), P5.03 (§20.1 items 2–4, §10.4),
  P5.04 (§20.1 item 5, §12.5, §19.4).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or
  run. No `npm install`, no `tsc`/`vite`/`vitest` execution. All code and
  tests are complete source written only; every test file carries an
  `UNRUN` header; TypeScript has NEVER been compiled for this tree (the
  human's first install/build/test is the verification point — the
  P5.01/01_25_scaffold_live.md discipline continues here). No ledger
  checkbox claims tested/verified/passed.

## 1. What was built

### P5.02 — Overview workspace (`console/src/workspaces/Overview.tsx` + `overviewData.ts`)

All five §20.1 item 1 elements, each with its §20.2 freshness:

| Element | Source contract | Honest display |
|---|---|---|
| Fleet availability | P2.18 current states + the mirrored P2.16 availability rule (`components/availability.tsx`) | Buckets per asset with the derivation statement; a failed per-asset state read is LISTED, never dropped |
| Current aggregate power | P2.18 `active_power`, GOOD/SUSPECT only | MW with the §10.2 sign label; typed `simulated` (part of the component); `null` total renders "NOT shown as 0 MW"; exclusions listed with reasons |
| Active schedules | P3.28 `ScheduleRevisionDto`, `state === 'active'` | ID + revision + horizon + roll trigger; "read at" timestamp (the record carries no served freshness field — stated) |
| Key alarms | — | The GAP state (see §2 below); no alarm semantics invented |
| Delivery status | P4.30 `DispatchCommandDto` counts per §14.2 state | ISSUED qualified "issued to the SIMULATOR — a simulated issue, not field delivery"; ACKNOWLEDGED qualified "application-layer acceptance; acknowledgement is NOT physical delivery" (wording reused from `32_handover_phase5.md` §2); the API's `simulation_only_statement` rendered verbatim |

Plus: the control-authority mode line (phase-boundary text — reading the
mode from configuration is P5.13/P5.27), the claims-derived role line, and
a quality/type sample table (frozen six states beside the §20.2 value
types). No demo data exists anywhere — no demo-mode flag is present
(§21.4 labelling is moot without fixtures; empty states are the default,
CONTEXT.md open question 6).

### P5.03 — Portfolios / Sites / Assets (`Portfolios.tsx`, `Sites.tsx`, `Assets.tsx`, `api/views.ts`)

| Workspace | Elements displayed | Source contract |
|---|---|---|
| Portfolios | composition counts, aggregation rules VERBATIM, commitments PLACEHOLDER (note verbatim, never upgraded), latest envelope with `envelope_version` + computation time (or the explicit absence) | P2.17 `PortfolioViewDto` (dto.rs); P3.12/P3.28 `EnvelopeRecordDto` (`computed_at` read defensively from the payload) |
| Sites | overview counts, PCC verbatim, grid-model reference + version/hash from the latest check evidence, edge connectivity VERBATIM (`simulator`/`not_connected`, `live:false` — never upgraded), local EMS gap state | P2.17 `SiteViewDto` (dto.rs); P4.30 check records for the P4.03 binding |
| Assets | metadata, lifecycle + deactivation reason, stored limits (labelled NOT runtime envelopes), maintenance/outage (stored registry field), per-value telemetry with the frozen six quality states + freshness + simulated flag, declared Q sign when present, derived availability (P2.16 mirror), asset alarms gap | P2.17 `AssetViewDto` + P2.18 `CurrentStateDto` (dto.rs) |

Role-specific visibility (P5.03 step 4): the session role renders from the
claims-derived store; visibility is API-enforced — a `forbidden` read
renders as an honest error block, never as an empty dataset. Two roles are
tested (operator vs viewer) in `Portfolios.test.tsx`.

### P5.04 — Grid workspace (`Grid.tsx` + `gridData.ts`)

- Every outcome row carries model id/version/content hash + completion
  timestamp (§20.2); a record without them renders explicit
  "not present in the record" markers — never a plausible version.
- Outcomes use the PERSISTED vocabulary (`PASSED | LIMIT_VIOLATED |
  FAIL_CLOSED`, `feasibility_check.rs`) — NOT the internal six-way routing
  of `verdict.rs`, which the gate renders before persistence.
- Margins (§12.5 step 7) and coverage (§12.5 step 6) render verbatim in
  distinct blocks; a PASSED record carrying a coverage statement is
  qualified **coverage-limited (uncovered parts are NOT validated)** — the
  ledger fixture pair (binding transformer limit / coverage limitation)
  is covered in `Grid.test.tsx`.
- FAIL_CLOSED renders the reason plus the not-inferred rule (§12.5 final
  paragraph, §17.5 invariant 9). NO feasibility-guarantee language
  anywhere (verdict.rs wording reused).
- Feeder/substation boundaries and P4.03 model health (lifecycle
  draft/active/retired; completeness/staleness/coverage) have NO serving
  API — both render the honest gap states, never an invented topology or
  health figure.

## 2. The alarm gap (P5.02's recorded input; P5.31 owns the resolution)

The Overview "Key alarms" panel renders exactly this state (test-pinned):

> Alarms: contract not yet defined (P5.31 will resolve). No alarm model
> exists in any prior phase — the frozen §15.3 event vocabulary contains
> no alarm event, and no Phase 1–4 API serves alarm records. This panel
> therefore shows NO alarms rather than fabricated ones; no alarm
> semantics are invented here (ledger P5.02 input note; P5.31 owns the
> minimal alarm model or its formal escalation).

No alarm field, severity scale, or alarm count exists anywhere in the
console; the same gap text surfaces per-asset in the Assets workspace.

## 3. Data-contract consumption map

| Console module | Consumes | Notes |
|---|---|---|
| `api/views.ts` (NEW mirrors) | dto.rs `PortfolioViewDto`, `SiteViewDto`, `AssetViewDto`, `EdgeConnectivityDto`, `CommitmentsPlaceholderDto`, `PortfolioCompositionDto`, `SiteOverviewDto`, `AssetLimitsDto` | snake_case wire names; drift-note discipline per types.ts |
| `api/{registry,telemetry,flexibility,dispatch}.ts` (P5.01, unmodified) | routes.rs / routes_phase2.rs / routes_phase3.rs / routes_phase4.rs read paths | tenant-scoped reads only; no tenant selector on the wire (§16.2) |
| `api/types.ts` (P5.01, unmodified) | dto.rs / dto_phase3.rs / dto_phase4.rs shapes | used as-is; `views.ts` adds only the P2.17 view shapes that were missing |
| `overviewData.ts` | §14.2 state vocabulary (counts only); P2.18 quality vocabulary | no state/quality invented server-side or client-side |
| `gridData.ts` | `FEASIBILITY_CHECK_OUTCOMES` (3 persisted outcomes); margins/coverage strings verbatim | internal verdict routing NOT re-derived |
| `components/availability.tsx` | `crates/flexcore-application/src/availability.rs` (P2.16 rule) | display MIRROR, branch-by-branch traceable; not a second definition |

## 4. Decisions (all DEFAULT)

- **D-P5.02-1 (DEFAULT):** the §20.2 value-type resolution is decided
  once in `ValueType.tsx` (`valueTypeFor`): ESTIMATED quality → `estimated`,
  STALE → `stale`, else the record's `simulated` flag → `simulated`/`measured`,
  else `untyped` (never fail-open to measured). The P5.10 workspace-wide
  pass may generalize this; the treatments are text-borne (not colour
  alone).
- **D-P5.02-2 (DEFAULT):** the aggregate composes per-asset current-state
  reads (no fleet-aggregate endpoint exists on the §15.2 surface) and
  admits only GOOD/SUSPECT numeric `active_power` values; zero
  contributors render as "no usable contributor", never 0 MW; every
  exclusion is listed. The per-asset read fan-out is a recorded
  limitation (page-size bound; re-visit when a fleet endpoint exists).
- **D-P5.02-3 (DEFAULT):** delivery status counts are plain persisted
  facts; the ISSUED/ACKNOWLEDGED qualifications reuse the Phase 4 handover
  wording VERBATIM, and the API's own `simulation_only_statement` is
  rendered rather than paraphrased.
- **D-P5.03-1 (DEFAULT):** the site grid-model version/hash come from the
  latest feasibility-check record for the site; the feasibility listing
  has no site filter, so the first returned page is filtered client-side
  and the consulted count is displayed (recorded incompleteness
  limitation).
- **D-P5.03-2 (DEFAULT):** the P2.16 availability rule is mirrored in
  `components/availability.tsx` with the source-connectivity input
  evaluated from the recorded pipeline property ("the signal is true at
  every pipeline emission point" — availability.rs); the limitation (no
  live connectivity observation at the viewer's instant) travels IN the
  rendered basis text. STALE/MISSING render "no new signal" — never an
  invented state.
- **D-P5.03-3 (DEFAULT):** no demo data anywhere in these workspaces, so
  no demo-mode flag exists (§21.4: labelling applies to fixtures; empty
  states are the default). The word "synthetic" appears only inside
  fixture data used by the UNRUN tests, never in runtime display strings.
- **D-P5.04-1 (DEFAULT):** the persisted three-outcome vocabulary is the
  display contract; the internal REJECTED/CONSTRAINED/… routing of
  verdict.rs is NOT re-derived client-side (that would invent outcomes the
  record does not carry).
- **D-P5.04-2 (DEFAULT):** coverage text is displayed verbatim without
  parsing; the coverage-LIMITED qualification keys on the mere PRESENCE of
  a coverage statement on a PASSED record (the §12.5 step 6 contract:
  "coverage statement/limitations, when produced"), so a
  coverage-limited record can never render as an unqualified pass.
- **D-P5.02-4 (DEFAULT):** tests render via `react-dom/server`
  `renderToString` (inside the EXISTING react-dom dependency) instead of a
  DOM test environment: @testing-library and jsdom are NOT in the scaffold
  package.json, the install ban forbids adding them, and a server render
  covers the honesty-labelling assertions. The hook-driven fetch itself is
  tested at the loader level (`loadOverview`/`loadPortfolios`/`loadSites`/
  `loadAssets`/`loadGrid` with a fake `ApiClient`), which also pins the
  exact §15.2 wire paths and the no-tenant-selector rule.

## 5. Honest limitations

- **Nothing was built or run.** TypeScript was never compiled; no vitest
  suite ran; no ESLint pass ran. The strict-`exactOptionalPropertyTypes`
  config has never type-checked ANY of this tree (P5.01 limitation
  carried forward); the human's first install/build/test is the gate.
- The API-layer type mirrors remain hand-written (the P5.01 generated
  client + OpenAPI drift test is still deferred to the first install);
  `api/views.ts` extends that interim discipline.
- Per-asset fan-out reads (Overview, Assets) and the client-side site
  filter (Sites) are bounded by the first returned page; the displays
  state this rather than claiming completeness.
- The P2.16 availability derivation cannot observe source connectivity at
  the viewer's instant (see D-P5.03-2); the derivation statement travels
  with every availability display.
- Feeder/substation boundaries and P4.03 model health are gap states by
  contract absence (`/api/v1/grid-models` not served; no model-health read
  API) — not deferred work claimed as present.
- The alarm gap remains OPEN until P5.31 resolves it; the Overview and
  Assets alarm panels are the explicit unavailable states it requires.
- Registry changes are additive only: the five implemented entries keep
  their P5.01 export names (`OverviewWorkspace`, …), so `App.tsx` and the
  router are structurally untouched; the remaining seven workspaces stay
  honest stubs (wave 2b/3 ownership).
