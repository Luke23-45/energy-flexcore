# Operator guide — Neryva FlexCore console (P5.29 guide source)

- **Status:** WRITTEN SOURCE, reviewed against the built console sources only —
  nothing in Phase 5 was installed, built, or run (loop bans, `ENTRY_NOTE.md`).
  Every display statement below was checked against the console source files
  it describes; the human's first build/run is the verification point. The
  non-author review (ledger P5.29 step 4) is **recorded as OPEN — not yet
  performed**; it is a human gate, not a formality this loop could self-issue.
- **Audience:** the operator of the v0.3.0-line console. This guide describes
  the console AS BUILT (the sources in `console/src/`), not a future one.
- **Evidence:** each section cites the phase decision record(s) that own the
  behaviour. The report sections are cited as §N per the plan report v1.1.
- **Companion:** the console README (`console/README.md`) records the
  toolchain and its UNRUN status; this guide records the MEANING of what the
  operator sees.

## 1. What this console is — and what it can never do

The console is the operator's **view** of the simulation-only 0.x platform
(report §7.5, §21.4). Read this list before anything else:

- **No field control.** The console never issues a command to a device, in
  this phase or any release. Every issued command in the platform is issued
  to the SIMULATOR and nothing else; there is no device write path (the
  Phase 4 boundary statement is rendered verbatim on the Dispatch
  workspace; `13_26_honesty_security.md`, P4.32 handover §2).
- **No execute endpoint.** The server exposes no generic execute endpoint
  (§15.2) and the console adds none. There is no "run this" button that
  dispatches anything.
- **No invented data.** Every displayed value comes from a tenant-scoped API
  read of platform records. No demo, sample, or synthetic filler data exists
  anywhere; empty and missing things are labelled as such
  (`13_26_honesty_security.md`; `07_31_market_plugins_admin_alarms.md`).
- **Acknowledgement is not delivery.** When you see ACKNOWLEDGED, that is
  application-layer acceptance by the simulator — never physical delivery
  (§14.1).
- **Simulation only.** The control-authority mode for this phase is
  Simulation / read-only (shadow). Shadow agreement is not a safety claim;
  the console makes no safety, certification, or protection-grade claim of
  any kind (AGENTS.md rules 2–3).
- **Steady-state results are not stability evidence.** Nothing shown in the
  Simulation Lab (or anywhere) may be read as transient frequency-stability
  evidence; HELICS coordinates federates, it does not replace their physics
  (`17_scoping_decision.md`, `16_helics_cosim.md`).

## 2. The persistent chrome (visible on every workspace)

| Element | What it tells you | Evidence |
|---|---|---|
| Control-authority banner | The 0.x phase boundary in text. The mode indicator shows `Simulation` from the console's recorded configuration constant — in this phase no mode-serving API exists, so the value is never taken from user input; reading the mode from deployment + per-site configuration is P5.27 (pending). | `13_26_honesty_security.md`; ledger P5.27 |
| Tenant banner | The tenant id and role of the authenticated session (claims-derived, §16.2), plus on-screen notes: where the value comes from, that no tenant selector exists, and that the indicator shows even on single-tenant deployments (it is the proof the context is what you think it is). | `32_33_tenant_live_ordering.md` |
| Live-update status line | "Live updates: connected" or "Live updates paused — data may be stale" with the last update time. A paused state is never silently resumed as if nothing happened; the console re-syncs explicitly from the REST read APIs after a gap. | `01_25_scaffold_live.md`; ledger P5.25 |
| Value-type legend | The "How to read the values" table (§2.2 below) as a UI element — the treatments are on screen, not tribal knowledge. | `10_12_design_pass.md` |

## 2.2 How to read a value: type, quality, freshness (§20.2)

Every displayed value carries three things, as literal text (never colour
alone):

1. **Value type** — one of:
   - `[measured]` — telemetry quality GOOD/SUSPECT/BAD/MISSING and the
     record's simulated provenance flag false (never the case on the Phase 2
     surface);
   - `[estimated]` — telemetry quality ESTIMATED;
   - `[stale]` — telemetry quality STALE;
   - `[simulated]` — the record's simulated provenance flag is true (every
     Phase 2 telemetry value on this console is `[simulated]`);
   - `[untyped value — type not derivable; never read as measured]` — the
     record carried something the resolver could not type. The resolver is
     decided once in `components/ValueType.tsx` (`valueTypeFor`) and pinned
     by tests; a per-screen restyle is a vocabulary defect.
2. **Quality state** — for telemetry, exactly one of the frozen six
   (§10.4), rendered verbatim: `GOOD`, `SUSPECT`, `BAD`, `MISSING`,
   `ESTIMATED`, `STALE`. An unknown code renders as `UNKNOWN(<code>)` —
   it is never mapped onto a plausible state.
3. **Freshness** — `event time <t> (age <a>)` and/or the console's
   `read at <t>`. A missing timestamp renders "not present in the record",
   never "now".

## 2.3 How to read a command: the 14-state lifecycle (§14.1–§14.2)

The Dispatch and Audit workspaces render the frozen vocabulary exactly (P0.11
spellings). Each state is a distinct bracketed marker + label + meaning — you
can tell the state from the display alone, without a tooltip (§20.2). The
states, in frozen order:

| State | Marker | What it means on this console |
|---|---|---|
| PROPOSED | `[proposed ·]` | Candidate setpoint. Proposed values carry a "proposed" treatment until ACKNOWLEDGED — never displayed as executed. |
| VALIDATED | `[validated ·]` | The VALIDATION gate passed (schema, freshness, capability, ramp, SoC, policy). One of three DISTINCT gates. |
| FEASIBLE | `[feasible ·]` | The FEASIBILITY gate passed (network model check, §12.5). Not operator authorization. |
| APPROVED | `[approved ·]` | The AUTHORIZATION gate passed (actor, scope, limits, expiry). Still a proposed value. |
| ISSUED | `[issued to simulator »]` | Issued to the SIMULATOR. "Issued" never means "issued to device". Labelled SIMULATED. |
| ACKNOWLEDGED | `[accepted by simulator (simulated) »]` | Application-layer acceptance by the simulator. NOT physical delivery. |
| TRACKING | `[tracking (simulated) »]` | Measured response observed from simulated telemetry; measured values come only from telemetry. |
| COMPLETED | `[completed (simulated) ✔]` | The acceptance criterion was satisfied on the simulated run — one asset command, never a portfolio. |
| REJECTED | `[rejected ✕]` | One of the three gates rejected the command; the failing constraint and its evidence reference are shown, never merged across gates. |
| FAILED | `[failed ✕]` | A frozen-table trigger fired after issue (NACK, timeout, transport, no response, tracking deviation/timeout) on the simulated path. |
| EXPIRED | `[expired ⏱]` | The validity window ended before the milestone was reached. |
| CANCELLED | `[cancelled ⌫]` | Explicit operator/policy cancellation — DISTINCT from SUPERSEDED, never merged. |
| SUPERSEDED | `[superseded ⇄]` | Replaced by a newer revision; the record is retained and retrievable, never hidden. |
| FALLBACK | `[fallback ⇅]` | A FAILED/EXPIRED command for which the asset/site fallback policy activated (policy reference carried as evidence). |

An out-of-vocabulary state string renders verbatim with an explicit unknown
marker — the console never guesses.

## 2.4 Other closed vocabularies you will see

- **The three gates** (§14.1): `validation`, `feasibility`, `authorization` —
  always shown as distinct evidence, never merged.
- **Solver outcomes** (§12.6, verbatim): `optimal`, `feasible`, `infeasible`,
  `timeout`, `numerical_failure`, `unsupported`. Never conflate classes: a
  HiGHS record is LP/QP/MIP, not SOCP; a Clarabel record is convex conic,
  not MISOCP; an IPOPT outcome is local, not a global-optimality guarantee.
- **Schedule-revision states**: `active` | `superseded` (superseded revisions
  stay visible and retrievable). **Roll triggers** (P3.24, verbatim):
  `initial_solve`, `horizon_advance`, `envelope_invalidation`,
  `asset_opt_out_dropout`, `forecast_update`, `operator_request`.
- **Declared objective** (P3.14): exactly one,
  `SYNTHETIC_ENERGY_COST_DEVIATION_V1`, provenance `SYNTHETIC` (D1/D2 OPEN —
  there is no market objective). Objective selection means choosing among the
  declared set; free-text objectives do not exist here.
- **Alarm codes** (ALARM-MODEL-0.1.0, P5.31 — derived-on-read from existing
  facts, never invented): `TELEMETRY_LOSS`, `TELEMETRY_STALENESS`,
  `FEASIBILITY_LIMIT_VIOLATED`, `FEASIBILITY_FAIL_CLOSED`,
  `DISPATCH_FAILURE`, `DISPATCH_EXPIRY`, `DISPATCH_FALLBACK`. Each alarm row
  carries its derivation rule and evidence source.
- **Feasibility outcomes** (per record): `PASSED | LIMIT_VIOLATED |
  FAIL_CLOSED` — a within-limits outcome is evidence against the declared
  model, not a field-safety or approval claim (§12.5).

## 3. The workspaces, one by one (§20.1)

Each entry states what the workspace shows, what its gaps are, and the owning
decision record. What is NOT shown is as important as what is: the console
states gaps instead of filling them.

### 3.1 Overview (§20.1 item 1)

Fleet availability, current aggregate power (MW, `P > 0` = export at the
point of connection, sign labelled), active schedules (ID + revision), key
alarms, delivery status — each with freshness. The aggregate never shows a
missing fleet as 0 MW: every excluded asset is listed with its reason.
Alarms come from the P5.31 derived-on-read contract (`GET /api/v1/alarms`):
the real derived list, an honest empty state, or an honest error block — no
fabricated alarm. Delivery status reuses the Phase 4 wording: every issue is
to the SIMULATOR; acknowledgement is NEVER physical delivery.
(`02_04_workspaces_a.md`; `07_31_market_plugins_admin_alarms.md`.)

### 3.2 Portfolios (§20.1 item 2)

Portfolio composition (site/asset counts; aggregation rules rendered
verbatim), commitments, and the latest flexibility envelope with its
envelope version and computation time (P3.12). Gaps: the registry's
commitment field is a PLACEHOLDER and is rendered as one — it is never
upgraded into a commitment list; when the envelope payload does not carry a
computation time, the gap is stated rather than replaced by the viewer's
clock. (`02_04_workspaces_a.md`.)

### 3.3 Sites (§20.1 item 3)

Site overview, PCC (verbatim), grid model, edge connectivity, local EMS
status. Gaps: the registry carries a grid-model REFERENCE string only — the
version + hash come from the latest feasibility-check record for the site,
or the gap is stated; edge connectivity shows the simulator/lab state
honestly (`status: simulator | not_connected`, `live` always false in Phase
2) and is never upgraded to a live edge; NO contract supplies EMS state, so
the EMS panel is the stated gap — an "EMS connected" line would be
invented. (`02_04_workspaces_a.md`.)

### 3.4 Assets (§20.1 item 4)

Asset state and registry metadata (including stored maintenance/outage
status — a stored field, not a live maintenance system), capability profile
REFERENCE, latest telemetry with the per-value quality state and freshness,
availability (derived per the mirrored P2.16 rule), stored limits, and the
per-asset derived alarms. Gaps: the constraint data behind the capability
reference is the Phase 3 contract and is not served by a Phase 1–4 read
API — the reference is rendered verbatim and the boundary stated; the rated
magnitudes are NOT runtime capability envelopes. (`02_04_workspaces_a.md`.)

### 3.5 Grid (§20.1 item 5)

Feasibility-check records: model version + content hash + timestamp on EVERY
outcome (rendered even when a record omits them, as explicit "not present in
the record" markers), per-record outcome with the failing constraint /
fail-closed reason and the margins summary verbatim, coverage statements in
a distinct block (a PASSED record carrying a coverage statement is qualified
coverage-LIMITED — uncovered parts are never shown as passing). Gaps: the
candidate grid-models path (§15.2) is not served, so no boundary map can be
drawn; the P4.03 model-health axes (completeness/staleness/coverage) are not
exposed by any read API — the panel is the stated gap. NO
feasibility-guarantee language appears anywhere. (`02_04_workspaces_a.md`.)

### 3.6 Schedules (§20.1 item 6)

Schedule revisions per chain: ID + revision, horizon, roll trigger, stored
state, supersedes/superseded-by links, §12.6 candidate detail (solver +
version, provider + version, problem class, objective value with units,
input-snapshot reference, grid-model version). SUPERSEDED revisions remain
visible. Alternatives/comparison show per-candidate §19.4 metrics with the
P3.23 shared-input rule enforced in the component — candidates on different
input snapshots are labelled INCOMPARABLE. Gaps: no API endpoint serves the
P3.23 ScheduleComparisonRecord; comparability is derived client-side from
input-snapshot equality only, and that derivation is stated on-screen. There
is no free-text objective entry. (`05_09_workspaces_b.md`.)

### 3.7 Dispatch (§20.1 item 7)

Proposed commands with their 14-state lifecycle, the THREE distinct evidence
blocks (validation / feasibility / authorization), and rejection
explanations (the failing constraint + evidence references, never merged
across gates). The approval flow requires explicit confirmation (P5.11
high-impact list) and step-up re-validation (P5.26) — and then resolves to
the recorded MISSING-ENDPOINT GAP: routes_phase4.rs exposes READ-ONLY
dispatch-command resources, there is no transition endpoint, and the console
never invents one. The payload's no-execute and simulation-only statements
are rendered VERBATIM. Emergency actions: none exists in this phase (open
item P5.06) and the console offers none. Live `DISPATCH_*` events update the
same labelled badge — the SIMULATED qualification travels in the badge
component itself. (`05_09_workspaces_b.md`; `13_26_honesty_security.md`;
`32_33_tenant_live_ordering.md` for the live side.)

### 3.8 Market & Programs (§20.1 item 8)

What exists: the registry's market-region field, the P2.17 commitments
PLACEHOLDER (rendered verbatim), the declared synthetic objective. What does
not exist is stated as explicit gap statements: no market adapter, no
program signals, no market delivery evidence. Nothing is fabricated; no
synthetic commitment is ever presented as a market product (D1 OPEN).
(`07_31_market_plugins_admin_alarms.md`.)

### 3.9 Simulation Lab (§20.1 item 9)

Scenario runs with scenario id + version, seed, code version, and the
SIMULATED label; per-fault rows for fault-injected runs (what was injected,
what it caused, the platform's response, with evidence references — an
operator can state the fault, the affected assets, and the response from
the display alone); failure journals for failed runs; the alarms panel
mapping run outcomes onto the closed alarm vocabulary; the "clear
injections and re-run clean" affordance states the exact CLI command and
the honest gap (no console trigger API yet) — a documented action, never a
fake button. Gaps: NO run-record serving API exists in this phase — the
read fails honestly and the gap is rendered (RUN_RECORD_API_GAP); the
console never fabricates runs. Benchmark figures (P5.23) and load figures
(P5.24) are pre-registered templates with EMPTY measured cells until the
human's recorded runs exist — no figure is invented. Replay/reproducibility
verdicts appear only when the recorded runs exist (P5.18–P5.19; the
double-run proof is UNRUN). (`21_24_faults_visibility.md`; `18_20_replay_library.md`; `23_benchmark_report.md`; `24_load_measurement.md`.)

### 3.10 Plugins & Integrations (§20.1 item 10)

What exists: the two platform-internal simulator adapter integrations
(asset simulator, pandapower grid adapter) and the API-backed per-site
edge-connectivity report. What does not exist is stated: no plugin kernel,
no plugin SDK (Stage 7, v0.5.0), no installed plugin, no plugin registry
API. The §5.7 plugin-authority rules are rendered read-only; no plugin is
ever shown as able to bypass authorization, tenant isolation, audit,
command lifecycle, or final dispatch validation.
(`07_31_market_plugins_admin_alarms.md`.)

### 3.11 Audit & Reports (§20.1 item 11)

The complete audit trail via `GET /api/v1/audit-events` (+/{event_id}) with
the P4.36 filters and keyset pagination: actor, action, object, decision
outcome + reason reference, evidence reference, before/after REFERENCES,
distinct event vs ingest times, correlation id, correction linkage (a
correction is a NEW row; the original is never edited). The per-command
decision chain (§24 step 9) classifies records onto the chain stages only
where the action is a frozen §15.3 event name; anything else is verbatim +
unclassified. Schedule vs actual uses requested values from the persisted
command payload and measured values ONLY from telemetry (with quality +
simulated flag). Dispatch failures/expiries/fallbacks with evidence links.
Export is a client-side JSON export of the CURRENTLY LOADED records that
preserves every field including references; the server has no export
endpoint by design. Gaps: the console computes NO tracking verdict (the
P4.16 acceptance-criterion record has no console read endpoint — recorded
gap). (`05_09_workspaces_b.md`.)

### 3.12 Administration (§20.1 item 12)

What exists: the P2.12 tenant read, the mirrored closed action vocabulary
(with its drift note), the P4.30 retention configuration SHAPE with unset
defaults, the D4 OPEN status line. Stated gaps: no production identity
integration, no deployment-configuration read API for retention values, no
real edge enrollment (the lab state is shown honestly).
(`07_31_market_plugins_admin_alarms.md`.)

## 4. How to read a rejection (§20.2; P5.06)

1. Find the command in Dispatch; its state badge is REJECTED (or a FAILED /
   EXPIRED / FALLBACK terminal state — different words, different causes;
   they are never merged).
2. Read the three evidence blocks separately: the rejection names ONE gate
   (validation, feasibility, or authorization) and shows that gate's failing
   constraint with its evidence references (checked-at time and model
   version included). Evidence is never merged across gates.
3. The evidence links resolve to the same records the Audit & Reports
   workspace shows (same ids, same correlation id — §21.3).
4. A rejection is gate evidence, not an alarm (P5.31): REJECTED commands map
   to no alarm code.

## 5. How to read the audit trail (§24 step 9; P5.09)

- Every record shows actor, action, object, decision outcome WITH its reason
  reference (never a bare allow/deny), evidence reference, before/after
  REFERENCES, event time vs ingest time (distinct, never merged), the
  correlation id (the same id a displayed dispatch state links to, §21.3),
  and correction linkage. The trail is append-only; the append-only
  statement travels in every response and is rendered verbatim.
- The decision chain for one command reads schedule → validation evidence →
  feasibility evidence → authorization decision → issue → acknowledgement →
  tracking → outcome. On this console the issue/acknowledgement/tracking
  stages are simulator-side and labelled SIMULATED (acknowledgement is
  application-layer acceptance, NOT physical delivery).
- Export preserves every field including references — it is the same
  evidence, not a summary that drops references.

## 6. Scenarios, replay, and fault-injected runs (§19.2; P5.18–P5.22)

- A scenario is a pinned, replayable record (inputs, versions, seeds,
  pre-registered expected outcomes/tolerances). Replay re-executes from the
  recorded inputs ONLY; a missing input is an error, never a silent default.
- Reproducibility means: same scenario + versions + seeds → same results
  within the PRE-REGISTERED tolerance. The verdict is shown only when the
  recorded double-run exists; it is UNRUN in this phase (the human's gate) —
  the Simulation Lab shows no invented verdict.
- A fault-injected run is marked FAULT-INJECTED and stays marked: per fault
  you can read what was injected, what it caused, and the platform's
  response, with evidence references. A fault-injected result is never
  presented as a clean one; the clean re-run affordance documents the CLI
  command (no console trigger API exists yet).
- All Simulation Lab results are labelled simulated, with the scenario and
  version they came from.

## 7. Live updates (§15.1, §15.4; P5.25, P5.33)

- The stream is AT-LEAST-ONCE and UNORDERED; the envelope says so
  (`delivery: at-least-once` + the ordering statement, rendered honest
  contract, not noise).
- Duplicates and reordered events cannot move your display backwards: the
  console deduplicates by event identity and applies per-entity
  last-write-wins by event_time (per event type: telemetry per site/asset,
  schedule state per portfolio/site/asset, dispatch lifecycle per command;
  events outside the three live areas are dedup-only standalone facts).
  Every discard is COUNTED in the ordering diagnostics — nothing is
  silently dropped. (`32_33_tenant_live_ordering.md`; the P4.24
  idempotency/sequence discipline, reused as a discipline.)
- On a RESYNC_REQUIRED notice or any disconnect: the console shows the
  paused-stale state, drops its local ordering ledger, and re-fetches from
  the REST read APIs. No gap is ever filled silently.
- A sequence GAP is a diagnostic (possible loss → re-sync), never a UI
  freeze.

## 8. Tenancy (§16.2; P5.26, P5.32)

- The tenant banner shows the authenticated session's tenant and role on
  every workspace. It comes from claims ONLY — never a URL parameter or a
  request field — and it shows even on a single-tenant deployment.
- There is NO tenant selector on this console, and none may be added
  without re-deriving the full context from a fresh authenticated session
  (the structural tests pin this).
- Where a displayed record carries its own tenant_id, the console can
  contrast it with the session tenant; a mismatch renders the visible
  `[TENANT MISMATCH — actions blocked]` cue and blocks console-side
  actions. The server denies and audits cross-tenant access
  independently — the banner is the display of the context, not its
  enforcement.

## 9. Role model (§17.2–§17.3; §20.2; P5.26)

Four closed roles (policy.rs spellings), evaluated deny-by-default; the
console's gate is UX only — the API enforces every action:

| Role | Sees / does (console surface) |
|---|---|
| platform-admin | Full read surface incl. Administration; administrative actions per the recorded grant sets. |
| tenant-admin | Tenant-scoped read surface; the only role with `command.approve` (site/asset-scoped) in the recorded default policy set; export via `audit.event.read`. |
| operator | Tenant-scoped operational read surface (telemetry, schedules, dispatch views); no approval grant. |
| viewer | Read-only views, no mutating action gates. |

A denial renders its REASON (the recorded grants + drift note), never a bare
hide. Step-up approval (typed target + session re-validation) guards the
approval flow; REAL server-verified step-up (WebAuthn/MFA) is a
later-stage deployment requirement, not implemented in 0.x — no endpoint was
invented. (`13_26_honesty_security.md`.)

## 10. Browser matrix and distribution (§21.4)

- Target: current and previous major versions of Chrome, Edge, Firefox, and
  Safari (the §21.4 browser matrix, recorded as the test target at P5.01).
- The full-matrix browser/OS run is a §22.3 release-gate activity; CI runs
  the recorded level (`28_ci_gates.md`). No browser figure may be claimed
  for a browser not actually tested (P5.24/P5.12 discipline) — and no
  browser-matrix run has been executed in this phase (UNRUN).
- Distribution: a static bundle served by the control plane or a CDN; no
  native mobile or desktop client exists.

## 11. Known gaps the operator will actually see (the honest list)

- **No transition endpoint**: the Dispatch approval flow resolves to the
  recorded MISSING-ENDPOINT GAP after confirmation + step-up (`05_09_workspaces_b.md`).
- **No run-record serving API**: Simulation Lab reads fail honestly
  (RUN_RECORD_API_GAP) until a serving API exists (`21_24_faults_visibility.md`).
- **UNRUN suites**: every test in this phase is written, never executed —
  the human's first `npm install`/`npm test`/`cargo test` is the gate
  (`HANDOVER.md` wave rows).
- **No demo mode**: there is no demo data anywhere, by design — an empty
  panel means the underlying records are absent or unreadable, and the
  panel says which (`13_26_honesty_security.md`).
- **Mode from a constant**: the mode indicator reads the recorded console
  constant until P5.27 lands the configuration read (`13_26_honesty_security.md`).
- **No tracking verdict, no EMS state, no model-health axes, no boundary
  map, no market adapter, no plugin kernel**: each is a stated gap on its
  workspace (§3 above) — a gap statement is the console telling the truth,
  not a missing feature to click around.

## 12. Review record (ledger P5.29 step 4)

- **Non-author review: NOT YET PERFORMED.** The guide was written against
  the console sources in the same loop that built them; the ledger requires
  a non-author (operator or QA) read-through for comprehensibility. That
  review is recorded as OPEN and belongs to the human gate alongside the
  build/test pass. No review has been fabricated here.
- Guide-vs-console conformance method: every §3 claim was checked against
  the workspace source headers and their pinned constants
  (`components/stateVocabulary.ts`, `components/honesty.tsx`,
  `components/ValueType.tsx`, `components/tenantBanner.tsx`,
  `live/ordering.ts`); the vocabularies in §2.3–§2.4 were transcribed from
  those modules verbatim.
