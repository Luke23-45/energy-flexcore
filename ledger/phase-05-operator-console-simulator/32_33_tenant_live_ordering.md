# Decision Record — P5.32 (tenant-context display and tenant-switch guard), P5.33 (live-update ordering and deduplication)

- **Date:** 2026-11-12 (Phase 5 wave 6a-adjacent — the tenant-display/live-ordering pass; loop rules in `ENTRY_NOTE.md`).
- **Subtasks:** P5.32 (§16.2, §20.2), P5.33 (§15.4, §15.1, §21.3).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or run. No `npm install`, no `tsc`/`vite`/`vitest` execution. All code and tests are complete source written only; both test files carry an `UNRUN` header and the human executes them at first install. The two ticked ledger boxes read "implemented, awaiting human build/test" — neither claims tested/verified/passed.
- **File ownership honoured:** the twelve workspaces, `components/honesty.tsx`, `apps/control-plane/**`, and `crates/**` were READ-ONLY in this pass (P5.08/P5.27 own `SimulationLab.tsx` + the mode files concurrently — untouched here).

## 1. P5.32 — coverage against the ledger criteria block

1. **Persistent tenant indicator on every workspace (step 1).** NEW
   `console/src/components/tenantBanner.tsx` exports `TenantBanner`, rendered
   by the app shell (`App.tsx` — surgical edit: the P5.01-era inline tenant
   paragraph is REPLACED by the component; the `className="tenant-indicator"`
   styling hook and `role="status"` are preserved inside the component). The
   value comes from the claims-derived session store (`state/tenantContext.ts`)
   ONLY — never a URL parameter, never a request field, never a user
   selection — and the source note is rendered ON SCREEN (not tribal
   knowledge, §20.2). A missing session renders the explicit no-session
   state, never a blank line. The banner sits alongside the
   control-authority mode indicator in the persistent shell.
2. **Tenant-switch guard (step 2).** Verified: NO tenant selector exists on
   the wire — `state/tenantContext.ts` has no switch affordance (its header
   records that switching re-derives the FULL context from a fresh
   authenticated session), the P5.26 structural scans pin that only the
   authentication layer may call `setSession`, and no workspace contains a
   tenant assignment. The guard is implemented as (a) the structural
   guarantee — NEW scans in `tenantBanner.test.tsx` over every non-test
   source with comments stripped: no `selectTenant|switchTenant|setTenant|
   changeTenant|chooseTenant|TenantSelector` identifier anywhere; no tenant
   read from the URL (`URLSearchParams`/`location` patterns); no workspace
   `tenant=` assignment; `setSession` caller-freedom re-asserted under the
   guard's own lens — and (b) the on-screen statement
   (`NO_TENANT_SWITCH_NOTE`): a context change requires a fresh authenticated
   session that re-derives claims/role wholesale; a failed re-derivation
   leaves the previous tenant's data visible with a stale-context warning,
   never silently mixed data. Since a switch UI does not exist, there is no
   switch flow to confirm-guard — the structural ban IS the guard, and the
   store's wholesale-overwrite assignment makes a partially re-derived
   context impossible by construction.
3. **Wrong-tenant cue (step 1's visible cue, contrasted with the data).**
   Verified server-side that records carry `tenant_id` (dto.rs
   `PortfolioDto`/`SiteDto`/`AssetDto`; dto_phase3.rs records; dto_phase4.rs
   command records; dto_alarms.rs `AlarmRecord`). NEW exported helpers:
   `checkTenantScope(record, context)` → `match | mismatch | unverifiable`
   (a record without a tenant_id is UNVERIFIABLE — stated honestly, never
   faked as a match); `tenantActionGuard(...)` → BLOCKS actions on mismatch
   with a rendered reason (the P5.26 denial-reason discipline: never a bare
   block) and allows `unverifiable` WITH the caveat that server-side scoping
   is the enforcement (blocking every tenant-less record would fabricate a
   client-side authority the console does not have); `TenantMismatchBadge`
   renders the visible text cue `[TENANT MISMATCH — actions blocked]`
   (text-borne, not colour alone, P5.12).
4. **Single-tenant case (step 4).** `SINGLE_TENANT_NOTE` is rendered in the
   banner: the indicator shows even on a single-tenant deployment — it is
   the proof the context is what the operator thinks it is.
5. **Manipulated-request denial (step 3's API half).** NOT duplicated: the
   runtime proof is server-side (cross-tenant reads are 404 and audited per
   §16.2/P4.28 — cited, per the P5.26 recorded division). The console-side
   defence-in-depth is the P5.33 tenant drop (§2.6 below).

**Honest integration gap (recorded):** the MISMATCH badge and action guard
are EXPORTED and TESTED, but the twelve workspace record tables do not yet
render them per row — the workspaces were read-only files in this pass. The
integration point is per record-table row: `checkTenantScope(row.tenant_id,
getSession()?.tenantId)` → render `TenantMismatchBadge` on mismatch and pass
`tenantActionGuard(...).allowed` into the row's action gating. Until that
wiring lands, the persistent banner + the server's fail-closed scoping are
the operative protections, and this record says so.

## 2. P5.33 — coverage against the ledger criteria block

NEW `console/src/live/ordering.ts` (pure, no React) + additive wiring in
`console/src/live/useLiveEvents.ts`; tests in NEW
`console/src/live/ordering.test.ts`. The P4.24 discipline
(`rejection_duplicate.rs` / `dispatch_security.rs` `SequenceGuard`) is
reused as a DISCIPLINE — one effective application per identity, no silent
overwrite, every anomaly counted — not as code (the console cannot import
Rust test fixtures; cited, not duplicated).

1. **Per-stream sequence (step 1) — the honest partial.** The frozen §15.3
   payloads carry NO monotonic sequence number, and `LiveEventEnvelope`
   (routes_live.rs) adds none — a per-stream sequence the console could
   "require" does not exist on this wire. **DEFAULT (recorded as a gap, not
   invented):** the console keeps the last-APPLIED `event_time` per
   (stream, entity) — the honest equivalent of the last-applied sequence —
   and discards events strictly OLDER than it. A real source-sequence
   requires an envelope field the closed vocabulary does not have; if the
   envelope ever gains one (or the outbox idempotency key), `ordering.ts`
   must adopt it in the same review (drift note in the module header).
2. **Identity and dedup (steps 1, 3).** Identity =
   `event_type + correlation_id + event_time` — the correlation fields the
   envelope actually carries. DRIFT NOTE: the platform's durable identity is
   the transactional-outbox `IdempotencyKey` (outbox.rs, unique constraint),
   but the SSE envelope carries NO idempotency field, so the console mirrors
   the correlation fields; every frozen payload has its own
   `correlation_id` (events.rs), so the tuple distinguishes distinct events.
   Dedup set is BOUNDED (`SEEN_LIMIT` 4096, oldest-evicted — a bound is
   mandatory on a long-lived stream; the eviction is tested, not hidden). A
   redelivered event: no visible change, no duplicate side effect, counted
   in `duplicatesDiscarded`.
3. **Stale-event rule per event type (step 2).** The rule table
   (`ORDERING_RULES`) states one rule per event type, each with its
   rationale, covering the CLOSED 26-tag vocabulary exactly once (tested by
   cross-checking the rule table against `stateVocabulary.ts`
   `FROZEN_EVENT_NAMES` via the deterministic PascalCase→SCREAMING_SNAKE
   conversion):
   - `TELEMETRY_RECEIVED`, `TELEMETRY_QUALITY_CHANGED` — telemetry stream;
     entity = site/asset; last-write-wins by `event_time`. Equal event_time
     with a NEW identity APPLIES (the recorded tie rule: concurrent facts at
     one event time cannot be ordered on this wire; the REST re-sync is the
     authority).
   - `SCHEDULE_PROPOSED`, `SCHEDULE_VALIDATED`,
     `FEASIBILITY_CHECK_COMPLETED`, `FLEXIBILITY_ENVELOPE_UPDATED` —
     schedule stream; entity = portfolio/site/asset; same LWW rule. The key
     is deliberately coarse (one anchor per scope, all four tags) — the
     display shows the freshest fact ABOUT the scope and each applied event
     carries its own event_type; **DEFAULT**, recorded.
   - All twelve `DISPATCH_*` — dispatch stream; entity = `command_id`
     (mandatory: a dispatch envelope without one is a CONTRACT DEFECT,
     never applied — the frozen payloads always carry it). A stale lifecycle
     event after a fresher one can never move the displayed state backwards.
   - The remaining eight tags (`ASSET_*`, `FORECAST_PUBLISHED`,
     `MARKET_SIGNAL_RECEIVED`, `PLUGIN_*`, `OPERATOR_ACTION_RECORDED`) —
     outside the three §15.1 live areas per the P5.25 classification:
     DEDUP-ONLY, every distinct event applies. These are standalone facts;
     the console merges no mutable per-entity state from them, so there is
     nothing to order — stated, not improvised.
   Stale events: discarded and COUNTED (`staleDiscarded`), with
   `lastDiscard` (what/why/when) observable — step 2's "not silently
   dropped". A sequence GAP is a diagnostic (the resync counters + the
   P5.25 re-sync), never a UI freeze (the ledger's risk-row mitigation).
4. **Contract-defect handling.** An unparsable `event_time`, an unknown tag
   (the vocabulary is CLOSED — live/types.ts drift note), or a dispatch
   envelope missing `command_id` is discarded as `discarded-contract-defect`
   and counted — never applied, never guessed around.
5. **RESYNC_REQUIRED handling (§15.4; P5.25 step 2).** `markResync` clears
   the identity set and the anchors so the re-fetched REST records are the
   new baseline; the discard counters are CUMULATIVE and survive (a reset
   must not launder the discard history). Wired additively in
   `useLiveEvents.ts`: on the `RESYNC_REQUIRED` notice AND on any
   paused-stale status, the hook resets the ledger; `seedAnchor` lets a
   consumer re-seed an entity's last-applied time from re-fetched records
   so a pre-gap redelivery cannot overwrite the re-fetched state
   (exported-for-use, like `DemoDataBadge` — no workspace wires it yet;
   recorded). The hook keeps `lastEvent` (raw, P5.25 contract unchanged)
   and adds `lastAppliedEvent` (gate-passed only) + `ordering` (the
   diagnostics snapshot). No workspace consumed `lastEvent` before this
   change (verified by grep), so nothing breaks.
6. **Defence-in-depth tenant drop (§16.2).** `applyOrderedEvent` takes the
   session tenant; an envelope whose `tenant_id` differs is dropped and
   counted (`tenantMismatchesDropped`). The server's per-caller tenant
   filter (routes_live.rs, fail-closed) is the ENFORCEMENT; this is a
   belt-and-braces client check that says so in its own outcome name and
   the module header.
7. **The at-least-once honesty note stays in the envelope.** The server's
   `delivery: "at-least-once"` + `ordering_statement` are untouched
   (server files read-only); the client still surfaces them, and the
   console README limit line ("ordering/dedup is P5.33") is now implemented
   client-side while the wire statement remains the contract for every
   consumer.

**Honest limitation (recorded):** this gate orders the console's DISPLAY
only. Platform-side deduplication (broker duplicate window + outbox
idempotency + consumer idempotency) and the P4.24 SequenceGuard remain the
enforcement; the REST re-sync is the authority whenever the client gate and
the server could disagree.

## 3. Decisions (labelled DEFAULT where they are this pass's choice)

1. **DEFAULT — per-entity `event_time` anchors instead of a per-stream
   sequence.** Forced by the wire (no sequence field exists); recorded as a
   gap with the adoption note, not silently substituted.
2. **DEFAULT — equal-event_time unseen identities APPLY (the tie rule).**
   Discarding them would drop concurrent distinct facts; the REST re-sync
   covers the ambiguity. Tested.
3. **DEFAULT — one coarse schedule anchor per portfolio/site/asset scope**
   across the four schedule-area tags. Coarseness stated in the rule's
   `statement` field and rendered/testable; the REST reads remain the
   authority.
4. **DEFAULT — dedup-only treatment for the eight tags outside the three
   §15.1 live areas**, following the P5.25 frozen classification
   (`classifyEventTag`) rather than re-classifying.
5. **DEFAULT — `unverifiable` tenant records: actions allowed WITH the
   enforcement caveat.** Blocking them would fabricate client-side
   authority; the API enforces tenant scoping on every request (§16.2).
6. **DEFAULT — structural (grep-based) tests with the method stated in each
   file header**, as in every prior wave: wiring in source, not runtime
   behaviour; the human's first `npm test` run is the gate.
7. **DEFAULT — bounded dedup memory (4096, oldest-evicted)** rather than an
   unbounded set; the eviction's effect is tested.

## 4. Honest limitations

- Everything is UNRUN. No test in this pass has ever executed.
- The MISMATCH badge is not yet rendered per workspace record row
  (workspaces read-only in this pass) — the integration point is recorded
  in §1.3.
- The structural scans prove source wiring, not runtime behaviour.
- The ordering gate cannot catch a stale event whose `event_time` is itself
  wrong (a clock-skewed future timestamp applies); event_time comes from
  the platform's P0.09 UTC time model, and the re-sync is the backstop —
  stated, not solved.
- `useCan`'s recorded non-reactivity note extends to the banner: the shell
  re-renders on route changes and the banner reads the store on render;
  session-swap reactivity lands with the app's session handling.
- P5.27 (mode enforcement) is a concurrent owner — nothing here touches the
  mode files.
