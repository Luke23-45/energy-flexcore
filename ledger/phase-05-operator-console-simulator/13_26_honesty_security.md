# Decision Record — P5.13 (simulation honesty in the console), P5.26 (console security: RBAC, tenancy, step-up approval)

- **Date:** 2026-11-12 (Phase 5 wave 5b — the honesty/security pass; loop rules in ENTRY_NOTE.md).
- **Subtasks:** P5.13 (§20.2, §21.4, §14.1, §14.2), P5.26 (§17.2–§17.4, §16.2, §20.2, §21.3).
- **Loop status:** NOTHING was installed, built, type-checked, linted, or run. No `npm install`, no `tsc`/`vite`/`vitest` execution. All code and tests are complete source written only; every test file carries an `UNRUN` header and the human executes them at first install. The two ticked ledger boxes read "implemented, awaiting human build/test" — neither claims tested/verified/passed.
- **TypeScript was never compiled.** Whether the tree type-checks cleanly is exactly what the human's first `tsc -b` proves.

## 0. Provenance — the interrupted agent's partials vs this pass's completion (traceability)

This wave was started by an agent that was quota-killed mid-task. Per the handoff
contract, everything it left was INSPECTED and VERIFIED before anything was
written here; nothing was restarted or duplicated.

**Left by the interrupted agent — verified against the sources and kept as-is
(all citations below were checked against the actual files, not assumed):**

- `components/honesty.tsx` — the P5.13 section was already present: the verbatim
  P4.32-handover boundary statement (`SIMULATION_ONLY_BOUNDARY_STATEMENT`, checked
  sentence-by-sentence against `32_handover_phase5.md` §2), the acknowledgement
  wording, the central per-state map `SIMULATED_STATE_MEANINGS`, the §21.4
  `ControlAuthorityModeIndicator` + its source note, the `SimulatedBadge`, the
  chain-stages note, and the live-label note.
- `components/honestyStructural.test.ts` — the structural guarantee test (method
  stated in its header; see §1 below).
- `components/stepUpApproval.tsx` — the complete step-up UI flow + honesty record.
- `security/{capabilityMatrix.ts, useCan.ts, denialLog.ts}` — the RBAC mirror,
  gate hook, and local denial log. The matrix was spot-checked against the
  authoritative Rust sources (`crates/flexcore-authz/src/action.rs` ids,
  `phase4_policies.rs` gate grants, `auth.rs` phase1–5 sets) and mirrors them
  faithfully, including the two recorded union-drift entries.
- Workspace wiring — Dispatch (mode indicator, boundary statement verbatim, live
  label note, capability-gated approval flow composing `ConfirmHighImpact` →
  `StepUpApproval` → the P5.06 missing-endpoint gap resolution, step-up denial
  logging), AuditReports (indicator + chain-stages `SimulatedBadge` + the
  `audit.event.read` gate), SimulationLab (indicator + run-level `SimulatedBadge`).
- Workspace-level test coverage for the P5.13 wording rules (the
  `simulatedMeaning` describe block in `Dispatch.test.tsx`).

**Inconsistent — found and fixed in this pass:**

1. `CommandStateBadge` did NOT render the central SIMULATED qualification, although
   `honestyStructural.test.ts` (the interrupted agent's own test) and the module's
   own `LIVE_DISPATCH_LABEL_NOTE` both stated the badge carries it — the badge
   rendered only the pre-P5.13 vocabulary definition meaning, so the test's
   render assertions (`SIMULATED` / `issued to the SIMULATOR` / no unqualified
   `issued to device`) could not hold. **Fix (surgical):** for the six
   simulator-side states the badge now renders the central
   `SIMULATED_STATE_MEANINGS[state]` entry INSTEAD of the vocabulary definition
   meaning (the map entry states the same semantics plus the `SIMULATED:` prefix,
   so nothing is lost); states before ISSUED keep their recorded definition
   meaning. This makes the badge — fetched or live-updated — carry the label from
   the ONE source, which is the actual P5.13 unification claim.
2. The mode-indicator render assertion in `honestyStructural.test.ts` expected a
   literal spanning a `{CONSOLE_CONTROL_AUTHORITY_MODE}` expression boundary;
   React SSR separates adjacent text nodes with comment markers, so the literal
   is not a stable substring of `renderToString` output. **Fix:** the assertion
   now pins `data-mode="Simulation"` plus the static prefix separately (with the
   reason recorded in the test).

**Missing — written in this pass:**

| Item | Content |
|---|---|
| `security/capabilityMatrix.test.ts` (NEW) | The mirror discipline (separation-of-duties entry, union-drift entries, forbidden §5.7 names, `live.stream.read` absence), `canAction` deny-by-default behaviour (unknown action, no session, outside-vocabulary role, no-grant role, scope-kind narrowing), and the authority-order note. |
| `security/useCan.test.ts` (NEW) | The gate over the claims-derived session store (no session denies; operator denied `command.approve`; tenant-admin allowed at asset scope; unknown action denies) and `useCanWithSession`'s claims fields for the step-up comparison. |
| `security/denialLog.test.ts` (NEW) | Bounded local recording + the AUDIT BOUNDARY pinned structurally (comment-stripped source scan: no `fetch`/XHR/sendBeacon/`Request` in code; the header cites P4.28/§16.2; the record shape carries no token/credential field). |
| `security/tenantScopingStructural.test.ts` (NEW) | The tenant-scoping structural guarantee (method below, §2.1). |
| `components/stepUpApproval.test.tsx` (NEW) | Strict token parsing, the pure `evaluateStepUp` (exact typed match, no-session denial, unparsable credential, per-claim mismatch naming, satisfied = typed AND re-validated), the LOCAL-only denial record, the honest panel rendering (pending/satisfied/no-session, the implementation note), and the structural composition with `ConfirmHighImpact`. |
| Ledger ticks + this record + the HANDOVER wave-5b row | Bookkeeping. |

## 1. P5.13 — coverage against the ledger criteria block

1. **Persistent control-authority-mode indicator on every workspace (§21.4).**
   `ControlAuthorityModeIndicator` renders `[Control-authority mode: Simulation]`
   with the recorded source note. The mode is a recorded console CONSTANT — never
   user input, never a URL parameter, never an API read (no mode-serving API
   exists in this phase); P5.27 owns reading it from deployment + per-site
   configuration and the enforcement. The structural test pins that Dispatch,
   AuditReports, and SimulationLab all render the ONE shared component.
2. **"Issued to simulator" / application-layer-acceptance wording.** Three layers
   render it: the state-vocabulary markers (P5.05-era, pinned by the P5.12
   agent's `statePrinciples.test.tsx` — untouched, read-only), the vocabulary
   definitions, and the CENTRAL `SIMULATED_STATE_MEANINGS` map that both
   `simulatedMeaning()` (Dispatch) and `CommandStateBadge` render (the
   unification fix, §0). The P4.32 handover boundary statement renders VERBATIM
   via `VerbatimStatement` at the Dispatch workspace level, and the control
   plane's own `simulation_only_statement` / `no_execute_endpoint_statement` DTO
   fields render verbatim per command where present — the console never
   paraphrases either.
3. **Console text review (automated).** `honestyStructural.test.ts` scans EVERY
   non-test `.ts`/`.tsx` source line-by-line for the banned phrases
   ("dispatched to", "executed", "delivered", "applied to device"); a violating
   line must be a wire identifier (`delivered_at`/`lastDeliveredAt`) or carry a
   simulator/simulated/never/not qualification on the same or adjacent line —
   which covers error, empty, and note strings too, because the scan is
   line-level over all sources. The scan was simulated against the current tree
   (Python, no test runner): **0 violations**. Recorded limitation: it is a
   heuristic over source lines, not a parse of the rendered DOM; the runtime
   wording is pinned separately by the rendered assertions in the same file and
   the workspace suites.
4. **Lifecycle-fixture display review.** The per-state meaning is asserted for
   the full §24 lifecycle in `Dispatch.test.tsx` (the `simulatedMeaning` block:
   ISSUED never "issued to device"/"delivered"/"executed on"; ACKNOWLEDGED is
   application-layer acceptance and NOT physical delivery; TRACKING/COMPLETED/
   FAILED/FALLBACK carry the SIMULATED qualification), and the badge renders the
   same map entries (the honestyStructural render assertions).
5. **Live stream.** `classifyEventTag('DISPATCH_*') === 'dispatch'` is pinned
   (the P5.25 stream can advance a displayed ISSUED/ACKNOWLEDGED/TRACKING badge);
   because the label travels IN the badge component (from the central map), a
   live-updated state renders labelled exactly as a fetched state. Dispatch
   renders `LIVE_DISPATCH_LABEL_NOTE` on-screen. Ordering/dedup for live events
   remains P5.33 (not this subtask).

**Structural guarantee test (method, stated per contract):** grep-based — the
test reads the console's own sources with `node:fs` and asserts (1) exactly one
source of the per-state SIMULATED wording, (2) the three workspaces import and
render the central module, (3) the automated banned-phrase review of §1.3,
(4) the badge carries the label for the simulator-side states and the live
classification routes `DISPATCH_*` to the dispatch area, (5) the one shared
mode indicator. What a structural scan cannot prove is recorded in the test
header: wiring in source, not runtime behaviour — the human's first `npm test`
run is the gate.

## 2. P5.26 — coverage against the ledger criteria block

1. **Tenant context from claims, never user input (§16.2).** The store
   (`state/tenantContext.ts`, P5.01) accepts only verified claims;
   `setSession` has NO caller anywhere in the console scaffold yet (the Phase 1
   auth bootstrap owns it when it lands) — pinned structurally. The api client
   sends credentials and NOTHING that could select a tenant (client.ts header
   discipline); no workspace imports the token renderer; no workspace contains a
   `tenant=` assignment. Method: `security/tenantScopingStructural.test.ts`
   scans every non-test source with line comments stripped (prose legitimately
   names the banned constructs — e.g. "never localStorage" — so the scan runs
   over code); the scan was simulated against the current tree: **0 violations**.
   The runtime cross-tenant proof is SERVER-side and deliberately NOT
   duplicated: cross-tenant reads are 404 by design and denials are audited on
   the P4.28 seam (`TenantDenialAuditRecord` / `SecurityAuditSink`; Phase 4
   handover §5) — cited, not re-implemented.
2. **RBAC on every console action (§17.2–§17.3).** `capabilityMatrix.ts` mirrors
   the recorded grant sets with verbatim `Action::id()`/`Role::id()` spellings;
   `useCan` derives the role from the claims-derived store and is deny-by-default
   exactly like the server (unknown action denies per P1.11; a missing session
   denies per P0.16 rule 5). The Dispatch approval flow is gated with
   `useCan('command.approve')`; a denial renders the REASON (recorded grants +
   drift note + the authority-order note) instead of a broken button, and the
   rendered note states the invariant-7 discipline: hiding the button is NOT the
   enforcement — the API enforces on every action. AuditReports gates the export
   surface with `useCan('audit.event.read')`. **THE ENUM IS AUTHORITATIVE:**
   `crates/flexcore-authz/src/action.rs` + the recorded grant sets win over the
   mirror on any drift; the mirror CAN DRIFT until hand-updated (no generated
   drift test exists in the console scaffold — the same recorded limitation as
   the Administration action-surface mirror, `07_31_market_plugins_admin_alarms.md`
   §2.3). The two recorded union disagreements (`command.read`, `feasibility.read`:
   REST read surface grants all four roles; the flexcore-authz gate set grants
   three) are recorded as a UNION with a drift note — never silently narrowed —
   and `command.approve` is recorded site/asset-scoped, tenant-admin only (the
   separation-of-duties default; no tenant-level approval grant exists).
3. **Step-up approval (§17.3).** `components/stepUpApproval.tsx` implements the
   UI flow ONLY, and says so on the panel: (a) an EXPLICIT typed confirmation of
   the action target id (exact, trimmed); (b) a fresh re-validation of the
   session credential on the dev-session path — the re-presented token must
   parse (strictly, no lenient fallback) to claims matching the authenticated
   session EXACTLY (tenant, subject, role). **REAL step-up authentication
   (server-verified re-authentication, WebAuthn/MFA) is a later-stage deployment
   requirement and is NOT implemented in 0.x** — no step-up validation endpoint
   exists on the §15.2 surface and NONE WAS INVENTED. The server-side gate
   remains the P4.13 authorization-gate records; a satisfied step-up is a LOCAL
   UI event with a local timestamp and the panel states it never grants
   authority. A failed attempt is recorded LOCALLY only
   (`security/denialLog.ts`, bounded ring buffer, no secrets in entries).
   **Merge point with the P5.10–P5.12 wave (recorded in both decision
   records):** `ConfirmHighImpact` (confirmation, P5.11) and `StepUpApproval`
   (step-up, P5.26) COMPOSE in the Dispatch approval flow — confirmation first,
   step-up inside its confirmed region, then the P5.06 missing-endpoint gap
   resolution (the console never performs an approval; no POST is invented).
   Neither wave edited the other's file; the composition is pinned structurally
   in `stepUpApproval.test.tsx`.
4. **Denial audit (§16.2, §21.3).** Server-side denials are the P4.28 records
   (cited above; not duplicated). The console adds ONLY the local UI-event log
   so diagnostics can explain what the user saw — `denialLog.test.ts` pins the
   boundary structurally (no network call in module code) and the no-secrets
   rule (action id, role, reason, surface, timestamp — nothing else).
5. **Emergency vs normal actions (§20.2).** No emergency action is defined in
   this phase; the P5.06 open item stands and the console offers none — the only
   console-triggerable high-impact action remains the recorded approval, behind
   confirmation + step-up (the high-impact list in `ConfirmHighImpact.tsx` states
   cancellation/supersede/fallback are NOT console-triggerable and why).

**Token storage honesty.** The session lives in the IN-MEMORY module store only;
nothing touches `localStorage`/`sessionStorage`/`document.cookie` (pinned
structurally). **DEFAULT** tradeoff, recorded in `useCan.ts`: an XSS cannot read
a PERSISTED token (there is none to steal; the dev-session token is rebuilt from
claims per request), but a refresh loses the session and re-authentication is
required each load. Hardened persistent storage (BFF/httpOnly-cookie patterns)
is a later-stage deployment requirement — not implemented in 0.x.

## 3. The honesty unification (what "single source" means concretely)

| Surface | Before this pass | After |
|---|---|---|
| `simulatedMeaning()` (Dispatch) | delegated to `SIMULATED_STATE_MEANINGS` (interrupted agent) | unchanged — one delegation |
| `CommandStateBadge` (fetched AND live-updated state) | vocabulary definition meaning only — INCONSISTENT with the module's own claims | renders the central map entry for the six simulator-side states (the fix) |
| Workspace-level notes | boundary statement + ack statement + chain note + live note, all central constants | unchanged |

Accepted redundancy (DEFAULT): `CommandDetailPanel` renders the badge's central
note AND the separate `simulated-meaning` paragraph — both trace to the SAME
constant, so there is no drift risk; the duplication is visual, not textual.
The state-vocabulary markers/definitions keep their P5.05 wording (pinned by the
read-only P5.12 suite) and are superseded ON DISPLAY for the six states by the
central entry; the definitions remain the recorded vocabulary.

## 4. Decisions (labelled DEFAULT where they are this pass's choice)

1. **DEFAULT — the badge carries the central SIMULATED entry instead of the
   vocabulary definition meaning for the six simulator-side states.** The map
   entry states the same semantics plus the `SIMULATED:` prefix; the
   unification claim (one source, fetched and live alike) outranks keeping the
   older rendering.
2. **DEFAULT — structural (grep-based) tests with the method stated in each
   file's header.** They prove wiring in source, not runtime behaviour; every
   such file records that limitation and the human's first `npm test` run is
   the gate. Chosen because the loop bans running a test environment.
3. **DEFAULT — step-up re-authentication is dev-session re-presentation.** The
   dev-session path is the only identity path shipped in Phase 1 (auth.rs,
   feature-gated); the honest alternative was inventing a step-up endpoint,
   which is a hard violation. Recorded as a later-stage deployment requirement.
4. **DEFAULT — `useCan('command.approve')` is called without a scope kind in the
   Dispatch flow.** The mirror's scope-kind narrowing is implemented and tested,
   but the approval flow does not yet derive the command's site/asset scope kind
   into the check; the recorded grants make the point moot for the default
   policy set (tenant-admin only), and the API enforces the actual scoped
   decision. A refinement, not a gap in enforcement.
5. **DEFAULT — SSR assertions never span an expression boundary** (React
   `renderToString` inserts comment markers between adjacent text nodes);
   assertions pin data attributes or stay within a single expression's text.

## 5. Honest limitations

- Everything is UNRUN. The banned-phrase scan and the four tenant-scoping scans
  were simulated with standalone Python against the current tree (0 violations
  each) — that is a source-level sanity check, NOT a test execution.
- The capability mirror can drift from the server enum until hand-updated; the
  drift note renders with every denial explanation.
- `useCan` does not subscribe to session changes (recorded in the module);
  reactivity lands with the app's session handling.
- The local denial log is diagnostics only and never leaves the browser; the
  §16.2 audit trail is the server's (P4.28).
- No step-up endpoint, no WebAuthn/MFA, no persistent token storage — all
  recorded as later-stage deployment requirements, none invented.
- The browser-matrix / keyboard-only / assistive-technology verification
  (P5.12's exit check, shared with P5.13's) remains the human's gate.
