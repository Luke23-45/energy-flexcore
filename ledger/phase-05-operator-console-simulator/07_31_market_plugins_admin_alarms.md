# Decision Record — P5.07 (Market & Programs, Plugins & Integrations, Administration) + P5.31 (the alarm model and contract)

- **Date:** 2026-11-11 (Phase 5 wave 3a; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.07 (§20.1 items 8, 10, 12; §25.2 D1/D5; §5 plugins — Stage 7),
  P5.31 (§20.1 item 1, §15.3, §20.2; P9.25 relation).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or
  run. No `npm install`, no lockfile, no `node_modules`, no `tsc`/`vite`/
  `vitest` execution, no `cargo build/check/test`. All code and tests are
  complete source written only; every test file carries an `UNRUN` header
  and the human executes them at first install/build. No checkbox in
  `ledger_phase_5.md` claims tested/verified/passed — the two ticked boxes
  read "implemented, awaiting human build/test".
- **TypeScript was never compiled** (carried limitation from waves 1–2b):
  the strict + `exactOptionalPropertyTypes` + `verbatimModuleSyntax`
  config has never type-checked ANY of this tree; the human's first
  `tsc -b` is the gate.
- **Rust was never compiled**: the new contracts/domain/authz/control-plane
  modules were written against the existing patterns (and the dependency
  rule was checked with `tools/check_dependency_rule.py` — see §6), but no
  `cargo check` has run; type-level defects are possible and the human's
  first build is the gate.

## 1. P5.31 — the alarm model: DEFINED, not escalated (the choice and why)

The ledger P5.31 offers two resolutions: define the minimal alarm model the
Overview workspace displays, or formally escalate it to an owning phase.
**DECISION: DEFINE** (the ledger's stated preference). Reasons:

1. Every alarm source the ledger's step-1 inventory lists EXISTS in this
   codebase by Phase 5 with a serving read path: the six §10.4 telemetry
   quality states (P2.18 current state), the persisted §14.2 command states
   (P4.10 record store), and the persisted §12.5 feasibility verdicts
   (P4.30 evidence store). Nothing new is needed to derive from them.
2. The sources that do NOT exist (market signals — no adapter, D1/D2 OPEN;
   edge/adapter health — P6.17, Stage 6; plugin health — no plugin kernel,
   Stage 7) get NO code, NO rule, and NO alarm: `absent_sources` names them
   in every response. No escalation was needed because no missing Phase 1/2
   event or API blocks the contract.
3. The frozen §15.3 event vocabulary was **NOT extended**: the 26-event
   freeze (P0.14; `flexcore-events::EVENT_COUNT == 26` is test-pinned)
   stays exactly as frozen — no `AlarmRaised`/`AlarmCleared` event exists
   or was added. Alarms are DERIVED RECORDS, exactly as the ledger requires
   ("Alarms are derived from events (§15.3), never invented by the
   console").

### 1.1 The model (`crates/flexcore-contracts/src/alarm.rs`, ADDITIVE)

| Element | Content |
|---|---|
| `ALARM_MODEL_VERSION` | `ALARM-MODEL-0.1.0 (P5.31)` — rendered by every consumer so a display cannot silently present a different model than the one reviewed. |
| `ALARM_MODEL_STATEMENT` | The verbatim operator-attention statement (rendered, never paraphrased, by the API response and the console panels): alarms are derived operator-attention records, **NOT protection signals**, never invented by the console, no §15.3 event. |
| `AlarmSeverity` (closed, DEFAULT) | `INFO` / `WARNING` / `CRITICAL` — display-priority labels; CRITICAL never implies a protection or safety class. |
| `AlarmCode` (closed, DEFAULT) | Seven codes, ONE per EXISTING source: `TELEMETRY_LOSS`, `TELEMETRY_STALENESS`, `FEASIBILITY_LIMIT_VIOLATED`, `FEASIBILITY_FAIL_CLOSED`, `DISPATCH_FAILURE`, `DISPATCH_EXPIRY`, `DISPATCH_FALLBACK`. No code for any source that does not exist. |
| `AlarmSourceKind` + `AlarmSource` | The traceable derivation source (kind + a stable reference to the exact record — the ledger's evidence link, fetchable from the §15.2 read APIs). |
| `AlarmState` | The ledger's `active/acknowledged/cleared` vocabulary; the derived-on-read path produces `ACTIVE` only (see §1.3). |
| `AlarmRecord` | `alarm_id` (deterministic composite key, no persisted UUID), tenant/site/asset scope (site/asset nullable), code, severity, state, deterministic message, `raised_at` (the source observation's **event time** — P0.09, never ingest time), `cleared_at` (always `None` here), `derived_at` (the derivation instant — distinct from both), source, `derivation_rule`. |

The severity-per-code mapping (`AlarmCode::default_severity`) is
deterministic and part of the reviewed model: TelemetryLoss WARNING,
TelemetryStaleness INFO, FeasibilityLimitViolated INFO,
FeasibilityFailClosed WARNING, DispatchFailure CRITICAL, DispatchExpiry
WARNING, DispatchFallback CRITICAL. No per-display choice exists.

### 1.2 The derivation rules (`crates/flexcore-domain/src/alarm_rules.rs`, ADDITIVE — pure functions, each citing its source)

| Rule (recorded id) | Input (source data, verbatim) | Output |
|---|---|---|
| `p5_31/telemetry_quality_alarm` | The §10.4 quality state of the P2.18 current-state observation per (asset, quantity) | BAD/MISSING → `TELEMETRY_LOSS`; STALE → `TELEMETRY_STALENESS`; GOOD/SUSPECT/ESTIMATED → no alarm (ESTIMATED is an explicitly tagged derivation, not a fault) |
| `p5_31/dispatch_outcome_alarm` | The stored §14.2 state of a P4.10 command record + the journal transition INTO that state | FAILED → `DISPATCH_FAILURE`; EXPIRED → `DISPATCH_EXPIRY`; FALLBACK → `DISPATCH_FALLBACK`; the in-flight chain and COMPLETED/REJECTED/CANCELLED/SUPERSEDED → no alarm (a REJECTED command is gate evidence, P5.06's display, not an alarm) |
| `p5_31/feasibility_outcome_alarm` | The persisted §12.5 outcome of a P4.30 check record | LIMIT_VIOLATED → `FEASIBILITY_LIMIT_VIOLATED`; FAIL_CLOSED → `FEASIBILITY_FAIL_CLOSED`; PASSED → no alarm |

All three are deterministic pure functions (same facts in, same record out
— the derived id makes the dedup property structural). A source record
whose journal lacks the transition into its current state is DEFECTIVE: it
is skipped AND counted (`skipped_defective_source_records`), never derived
with an invented timestamp.

**"Sustained", honestly:** the ledger words the telemetry rule as
"BAD/MISSING sustained ⇒ telemetry-loss alarm". A minimum-duration debounce
requires event-history processing the derived-on-read contract does not
perform; the implemented rule is the honest minimal form: the alarm exists
exactly while the current observation carries the qualifying state
(re-derivation without the condition yields no record — the alarm
self-clears by absence). Recorded as a limitation, not hidden.

**No protection-grade semantics:** the rules produce operator-attention
records and nothing else; the model statement (in every response and
panel) says so. AGENTS.md rules 2–3 hold: nothing here triggers, inhibits,
or supervises any physical action.

### 1.3 Derived on read — no migration (the recorded choice)

**DECISION: derived-on-read; migration 0018 NOT written.** The ledger says
an additive migration is NOT required if alarms are derived-on-read; the
derived-record contract with `AlarmState::ACTIVE`-only satisfies P5.31's
exit check (Overview displays real alarms per source; no fabricated alarm).
Consequences, recorded:

- No alarm persistence exists: `cleared_at` is never served,
  `active_only=false` is refused with an honest 422 (there is no alarm
  history to query), and there is no acknowledge endpoint (no alarm
  mutation path at all — `alarm.write`/`alarm.acknowledge` do not exist on
  the closed authz surface and were added to its reject tests).
- `AlarmState::Acknowledged`/`Cleared` remain DEFINED vocabulary values for
  the persistence + acknowledgement path a later phase owns (the ledger's
  contract list names acknowledged/cleared; the values exist so the later
  migration does not repurpose anything).
- A future persistence phase adds its own migration; nothing here would
  need renaming (the record already distinguishes event time, derivation
  time, and cleared time).

### 1.4 The API (`apps/control-plane/src/routes_alarms.rs` + `dto_alarms.rs`, NEW; additive wiring)

`GET /api/v1/alarms` — tenant-scoped, READ-only, on the exact P4.30 route
pattern (resolve identity 401 → authorize `alarm.read` 403 → derive inside
one unit of work → stable envelope):

- **Filters** (all validated, malformed = 422): `severity` (closed set),
  `code` (closed set), `asset_id` / `site_id` (UUID), inclusive
  `from_event_unix_millis`/`to_event_unix_millis` window on `raised_at`
  (inverted = 422), `active_only` (only `true`/unset valid; `false` =
  422 with the derivation-contract reason), `limit` (default 200, max
  1000 — the recorded convention, applied per consulted source listing).
- **Derivation inputs:** dispatch commands (three qualifying-state pages +
  per-command journal read for the transition time), feasibility checks
  (first page), asset registry page + per-asset telemetry current states
  (only when the telemetry read port is wired). Bounded-page consultation
  is a recorded limitation, stated in the response.
- **Response honesty fields** (`AlarmPageEnvelope`): `data` (severity-rank
  then alarm-id order), `sources` (consulted counts per source, the
  telemetry-port availability statement, skipped-defective count,
  `absent_sources` naming the three non-existent sources), `derived_at`,
  `derivation_statement` (verbatim), `model_version`, `next_cursor: null`
  (single bounded derivation — no cursor by contract).
- **Wiring (all additive):** `routes.rs` merges `alarms_router()`;
  `lib.rs` declares the two modules; `openapi.rs` registers the path +
  four schemas + the description sentence; `auth.rs` gains
  `phase5_default_policies()` (the cumulative Phase 4 set + `alarm.read`
  granted to ALL FOUR roles at tenant scope — the same discipline as the
  Phase 4 read surface; no role bypasses the tenant match).
- **Authz (additive closed-enum extension, the recorded rule):** one
  `Action::AlarmRead` variant + its `id`/`parse` arms + the round-trip
  test entry + the deny tests for `alarm.write`/`alarm.acknowledge`.
- **Tests** (`tests/api_alarms.rs`, NEW, UNRUN): the derivation contract
  per source (including good-data-yields-no-alarm and the telemetry rule
  through the wired port), the honesty fields, tenant isolation (no
  cross-tenant derivation), filter discipline (422s incl. the
  `active_only=false` refusal), the event-time window, least-privilege
  (401/403/all roles), and the GET-only OpenAPI/runtime surface check.

### 1.5 The console (`components/alarmsPanel.tsx` NEW; `Overview.tsx`/`Assets.tsx` surgical)

- The Overview "Key alarms" gap panel (`alarm-gap`) is REPLACED by the
  real panel consuming `GET /api/v1/alarms`: severity-borne rows with
  code, message, scope, event-time freshness, state, and the evidence
  link; the honest empty state ("No alarms are currently derivable … not a
  promise that everything is healthy"); an honest per-panel error block
  when the derivation cannot be read (a failed derivation is never an
  empty list); the consulted/unavailable/absent source inventory; the
  model-source citation rendered verbatim (`ALARM-MODEL-0.1.0 (P5.31)` +
  the statement) and the resolved-contract note (`ALARM_CONTRACT_TEXT`,
  replacing `ALARM_GAP_TEXT` in `overviewData.ts`).
- The Assets per-asset gap panel (`asset-alarm-gap`) is replaced by the
  per-asset filter of the same read (tenant-wide alarms cannot appear
  under an asset — tested).
- A failed alarms read never degrades the workspace's other elements: the
  alarm envelope rides alongside the existing reads as an independently
  caught sub-read (`alarms`/`alarmsError`).
- **Surgical discipline:** only the alarm-gap panels, the loader additions,
  the import lines, and the two honesty-comment lines changed in
  `Overview.tsx`/`Assets.tsx` (wave 2a's files); every other element and
  its tests are untouched. The two affected test files were updated ONLY
  where they pinned the old gap wording (`Overview.test.tsx`,
  `Assets.test.tsx`) plus new alarms fixtures/assertions.
- `api/alarms.ts` (NEW) mirrors `dto_alarms.rs` in wire names + pins the
  closed vocabularies; `api/alarms.test.ts` (NEW, UNRUN) pins the wire
  paths (snake_case filters, no tenant selector — §16.2) and the
  vocabulary mirrors.

### 1.6 The P9.25 relation (the consumer, not the definer)

P9.25 (pilot observability verification) verifies alerting signals at the
pilot — stale telemetry, command-acknowledgement timeout, solver failure —
with thresholds and recipients. It CONSUMES a signal vocabulary; it does
not define one. The P5.31 model gives its Stage 9 work a stable vocabulary
to map the pilot's alert catalogue onto; the mapping itself is P9.25's
scope and is not pre-empted here. No alert threshold, recipient, or
escalation is defined in this phase.

## 2. P5.07 — the three supporting workspaces (honesty-first)

### 2.1 Market & Programs (`MarketPrograms.tsx` + `marketData.ts` + test)

| §20.1 item 8 element | What is rendered | Source |
|---|---|---|
| Market region | The registry `market_region` field per portfolio, verbatim, labelled synthetic (D1 OPEN — the field is free text, never interpreted as a connected market) | P1.17 `PortfolioDto` |
| Program signals | **Gap state** (`program-signals-gap`): no program signal exists; the frozen §15.3 `MarketSignalReceived` has NO producer in 0.x (D1/D2 OPEN) — stated, never fabricated | inventory statement |
| Commitments | The P2.17 PLACEHOLDER rendered verbatim (`populated=false` + the registry note), never upgraded | P2.17 `PortfolioViewDto.commitments` |
| Adapter status | **Gap state** (`market-adapter-gap`): "No market adapter is configured — none exists in the 0.x simulation line" (the ledger's required wording) | inventory statement |
| Delivery evidence | **Gap state** (`delivery-evidence-gap`): no market delivery exists to evidence; the only "program" is the declared synthetic objective, executed against the simulator; the simulated command evidence chain is pointed to the Dispatch/Audit workspaces | inventory statement + the P3.14 declared objective (mirrored from `stateVocabulary.ts` — SYNTHETIC provenance, "not a market product … (D1/D2 OPEN)" rendered verbatim) |

Empty states exist for every element and are tested; no demo data exists
anywhere (the CONTEXT.md open-question-6 default).

### 2.2 Plugins & Integrations (`PluginsIntegrations.tsx` + `pluginsData.ts` + test)

| Element | What is rendered | Source |
|---|---|---|
| Installed plugins | **Gap state** (`plugins-gap`): no plugin kernel, no plugin SDK (Stage 7, v0.5.0 per the release ladder); 29_safety_invariant_matrix.md residual gap 3 quoted honestly — no plugin execution path exists, so nothing can be bypassed; no plugin is shown as installed | release ladder + the recorded matrix |
| Plugin authority rules | Read-only §5.7 statement: plugins can never select the control-authority mode, never bypass authorization/tenant isolation/audit/command lifecycle/final dispatch validation; WASM/WIT or out-of-process gRPC, never arbitrary Rust `dylib`; `dispatch.authorize`/`dispatch.issue` do not exist on the closed action surface (flexcore-authz `action.rs`) | §5.7 + action.rs |
| Registered integrations | Exactly the two that exist — the asset simulator integration (P5.14) and the pandapower grid adapter integration (P5.15) — each marked `no — platform-internal`, with the scope statement that neither is a plugin nor a registered extension | ledger 14_15_adapters.md |
| Site edge connectivity | The API-backed P2.17 record per site: `simulator`/`not_connected`, `live:false`, rendered verbatim and never upgraded (D5 OPEN) | P2.17 `SiteViewDto.edge_connectivity` |

There is NO plugin registry API on the §15.2 surface (no `/plugins`
resource exists), so the panel fabricates none — the plugin list is the
explicit empty state, which is the manifest-comparison check's honest
result.

### 2.3 Administration (`Administration.tsx` + `adminData.ts` + test)

| §20.1 item 12 element | What is rendered | Source |
|---|---|---|
| Identity integration | **Gap state** (`identity-gap`): no production identity provider — dev-auth only (auth.rs, feature-gated, never a production credential); the real OIDC session is P5.26; every shown value comes from claims/tenant-scoped reads (§16.2) | auth.rs |
| Tenants | The caller's OWN tenant (P2.12 read-only grants; GET /api/v1/tenants lists exactly it — no platform-wide enumeration), read-only statement, honest read-error state | P2.12 grants / routes.rs |
| Roles / policies | The four roles + the closed action vocabulary MIRRORED from `action.rs` (grouped by adding phase, incl. the P5.31 `alarm.read`), with THE ENUM IS AUTHORITATIVE drift note and the recorded limitation that no generated drift test exists for the console mirror; the §5.7 exclusions (`dispatch.authorize` etc.) listed as deliberately absent and asserted never to appear in the mirror | flexcore-authz `action.rs` |
| Data retention | The seven §16.3 classes as the RetentionPolicy configuration SHAPE, each `UNSET (not served by any API in this phase)` — configuration, never values; "No universal retention period is chosen in code (§16.3)" | P4.30 RetentionPolicy (flexcore-storage/retention.rs) |
| Edge enrollment | The lab state honestly: registry edge identity + P2.17 connectivity (`simulator`/`not_connected`, `live:false`) per site — no real edge agent is enrolled (D5 OPEN, first site integration v0.4.0) | P2.17 site views |
| License audit | The D4 status line: OPEN, an audit INPUT (the pinned dependency licenses in RESEARCH.md), no compliance conclusion implied | CONTEXT.md D4 row wording |

## 3. Decisions (all DEFAULT unless marked)

1. **DEFAULT — P5.31 is resolved by DEFINITION, not escalation** (§1).
2. **DEFAULT — alarms are derived records, derived on read; NO migration
   0018** (§1.3). The §15.3 freeze is not extended; no Alarm event.
3. **DEFAULT — the closed vocabularies** (3 severities, 7 codes) cover
   exactly the ledger's existing-source inventory; adding a source means a
   new code + rule + test (the closed-enum discipline, same as authz).
4. **DEFAULT — the deterministic alarm id** (`flexcore:alarm:<CODE>:<scope>`)
   instead of a persisted UUID: a derived-on-read record has no row to own
   a UUID, and the composite key gives same-facts-same-record for free
   (the dedup property the P5.33 discipline wants).
5. **DEFAULT — the alarm read grants to all four roles at tenant scope**
   (the Phase 4 read-surface discipline: alarms are tenant-scoped evidence;
   cross-tenant derivation is impossible by construction).
6. **DEFAULT — `active_only=false` is a 422 refusal, not an empty page**
   (an honest refusal of what the contract cannot serve).
7. **DEFAULT — the two console gap panels were replaced surgically**;
   the two affected test files were edited only where they pinned the old
   wording (wave-2a ownership respected).
8. **DEFAULT — the Administration action-surface mirror is hand-written
   with an explicit drift note** rather than generated: no codegen exists
   in the console scaffold, and the server enum is pinned by its own
   tests; the mirror's limitation is recorded on-screen and here.

## 4. Honest limitations

- **Nothing was built or run** (loop bans): no cargo build/check/test, no
  npm install, no tsc/vite/vitest/eslint. All tests are UNRUN.
- The alarm derivation consults BOUNDED pages per source listing (the page
  limit); a full-fleet derivation needs the fleet endpoint that does not
  exist (the same recorded limitation as the Overview aggregate).
- The "sustained" debounce is not implemented (§1.2) — the derived alarm
  exists exactly while the current observation holds.
- No alarm persistence: no history, no acknowledge flow, no cleared-at;
  the vocabulary values are defined but unreachable in 0.x (recorded for
  the later persistence phase).
- The telemetry alarm source serves only when the telemetry read port is
  wired; in the Postgres composition root it currently is NOT (the
  recorded P2.06-adjacent gap), so telemetry-derived alarms are absent and
  the response says so — never silently.
- The console action-surface mirror can drift from `action.rs` until
  hand-updated (the drift note is on-screen; no generated drift test).
- The registry comment block (`registry.ts`) still describes wave-2b+
  workspaces as stubs — it was already stale after wave 2b and was
  deliberately left untouched (minimal-edit rule; no behavioural effect).
- Defective source records (a command whose journal lacks the transition
  into its current state) are skipped AND counted in the response
  (`skipped_defective_source_records`); the count's operator-facing
  meaning is documented in the decision doc and the DTO doc comment.

## 5. Files created / modified (complete list)

**NEW (14):**
- `crates/flexcore-contracts/src/alarm.rs` (+ its unit tests)
- `crates/flexcore-domain/src/alarm_rules.rs` (+ its unit tests)
- `apps/control-plane/src/routes_alarms.rs`
- `apps/control-plane/src/dto_alarms.rs`
- `apps/control-plane/tests/api_alarms.rs` (UNRUN)
- `console/src/api/alarms.ts`
- `console/src/api/alarms.test.ts` (UNRUN)
- `console/src/components/alarmsPanel.tsx`
- `console/src/workspaces/marketData.ts`
- `console/src/workspaces/MarketPrograms.test.tsx` (UNRUN)
- `console/src/workspaces/pluginsData.ts`
- `console/src/workspaces/PluginsIntegrations.test.tsx` (UNRUN)
- `console/src/workspaces/adminData.ts`
- `console/src/workspaces/Administration.test.tsx` (UNRUN)

**REPLACED (P5.01 stubs, export names kept):**
- `console/src/workspaces/MarketPrograms.tsx`
- `console/src/workspaces/PluginsIntegrations.tsx`
- `console/src/workspaces/Administration.tsx`

**ADDITIVE EDITS (existing behaviour untouched):**
- `crates/flexcore-contracts/src/lib.rs` (module declaration + doc row)
- `crates/flexcore-domain/src/lib.rs` (module declaration)
- `crates/flexcore-authz/src/action.rs` (`AlarmRead` variant + id/parse
  arms + test entries + the two deny assertions)
- `apps/control-plane/src/auth.rs` (`phase5_default_policies`)
- `apps/control-plane/src/routes.rs` (merge the alarms router)
- `apps/control-plane/src/lib.rs` (two module declarations)
- `apps/control-plane/src/openapi.rs` (path + schemas + description)
- `console/src/workspaces/Overview.tsx` (alarm panel replacement + loader
  addition — surgical)
- `console/src/workspaces/Assets.tsx` (per-asset alarm panel replacement +
  loader addition — surgical)
- `console/src/workspaces/overviewData.ts` (`ALARM_GAP_TEXT` →
  `ALARM_CONTRACT_TEXT` + the header honesty line)
- `console/src/workspaces/Overview.test.tsx` (gap-pinned assertions
  updated + alarms fixtures — surgical)
- `console/src/workspaces/Assets.test.tsx` (same — surgical)
- `ledger/phase-05-operator-console-simulator/ledger_phase_5.md` (the two
  objective checkboxes)
- `ledger/phase-05-operator-console-simulator/HANDOVER.md` (wave 3a row)

**NOT touched (per file ownership):** `crates/flexcore-simulator/**`,
`apps/worker/**`, `apps/sim-runner/**`, `crates/flexcore-grid-api/**`,
`docs/foundations/**`, `docs/plan/**`, `.agent/**`, `live/`, `state/`,
`WorkspacePlaceholder.tsx` (still used by SimulationLab), `App.tsx`,
`router.ts`, `registry.ts` (ids/titles/export names unchanged).

## 6. Verification performed in this loop (actual output quoted in the final report)

- `python tools/verify_docs.py` — run (documentation integrity gates).
- `python tools/validate_agent.py` — run (Agent Skills / roles gates).
- `python -I tools/check_dependency_rule.py` — run (the new module edges:
  contracts+serde only for `alarm.rs`; domain→contracts only for
  `alarm_rules.rs`; the control-plane additions stay in the composition
  root).

Nothing was installed; nothing was compiled; no test was executed.
