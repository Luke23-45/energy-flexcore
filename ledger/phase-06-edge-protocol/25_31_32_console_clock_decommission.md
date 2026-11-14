# P6.25 + P6.31 + P6.32 — Operator console surfacing, edge clock-quality monitoring, and edge decommissioning: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P6.25, P6.31, P6.32, Phase 6 ledger
(`ledger_phase_6.md`). Owner roles: frontend engineer (P6.25), security +
Rust engineer (P6.31/P6.32). Depends on: P6.15 verifier
(`15_16_command_verify_forward.md` — the clock-quality parameter seam),
P6.14 gate (`13_14_17_adapter_command_policy.md`), P6.06 identity/enrollment
(`06_07_identity_channel.md`), P6.09 buffer (`08_09_ingest_buffer.md`),
P6.18 audit buffer (`18_21_audit_offline_isolation.md`), P6.03 D6 boundary
review (`03_d6_boundary_review.md`), the Phase 5 console components.

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** the code and tests in this
wave are WRITTEN, NOT BUILT and NOT RUN. No `cargo`, `npm`, `tsc`, or test
runner was executed. Every test below is UNRUN until the human build/test
pass executes it. Nothing in this document claims a test passed, a build
succeeded, or a measurement was taken.

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `crates/edge-agent/src/clock_quality.rs` | NEW (P6.31) |
| `crates/edge-agent/tests/clock_quality_p6_31.rs` | NEW (P6.31) |
| `crates/edge-agent/src/lib.rs` | EDITED (additive `pub mod clock_quality;` anchored immediately after `pub mod uplink_mtls;` only) |
| `crates/edge-agent/src/health.rs` | EDITED (ADDITIVE ONLY: one new `HealthReason::ClockDrift` variant + its token arm + the header note; no existing variant, derivation rule, or test changed) |
| `crates/edge-agent/src/enrollment.rs` | EDITED (ADDITIVE ONLY: the P6.32 decommission section appended after `EnrollmentOrchestrator`; two import lines extended) |
| `crates/edge-agent/tests/decommission_p6_32.rs` | NEW (P6.32) |
| `apps/control-plane/src/routes_edge.rs` | EDITED (ADDITIVE ONLY: the P6.25 status source seam + `GET /api/v1/edge/status`, the P6.32 decommission endpoint, in-module tests; the existing enroll route, resolver, and tests untouched) |
| `apps/control-plane/src/dto_edge.rs` | EDITED (ADDITIVE ONLY: the P6.25/P6.32 DTOs appended) |
| `apps/control-plane/src/state.rs` | EDITED (ADDITIVE ONLY: the `edge_status` field, its default, and a `with_edge_status` builder) |
| `apps/control-plane/src/openapi.rs` | EDITED (ADDITIVE ONLY: the two new paths + schemas registered) |
| `console/src/api/edgeStatus.ts` | NEW (P6.25) |
| `console/src/workspaces/sitesEdgeStatus.ts` | NEW (P6.25) |
| `console/src/workspaces/SitesEdgeStatus.tsx` | NEW (P6.25) |
| `console/src/workspaces/SitesEdgeStatus.test.tsx` | NEW (P6.25, UNRUN) |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | EDITED (P6.25/P6.31/P6.32 ticks only) |
| this file | NEW (decision record) |

## 1. P6.25 — operator console surfacing (display-only command-path guarantee)

**Server seam first.** `GET /api/v1/edge/status?device_id=<id>` (ADDITIVE
path per the recorded P4.36/P5.31 rule; registered in `openapi.rs`):

- **Auth (recorded drift, the P5.25 `authorize_live_stream` pattern):** the
  closed `Action` enum is owned by `crates/flexcore-authz` (outside this
  wave's ownership), so the route evaluates `telemetry.read` (tenant-scoped,
  all roles) as the interim deny-by-default gate; the swap point is one call
  site, documented in the handler.
- **Data source (the honest seam):** the control plane holds NO edge state
  of its own (the edge is outbound-only, §3.3). Operational facts
  (last-seen, certificate expiry, adapter-health snapshot, buffer depth,
  freshness, command-path state) arrive through the `EdgeStatusSource` seam;
  the shipped default `NoEdgeStatusReported` renders EVERY operational field
  as an explicit gap (`null` + the note) — never a plausible "connected" or
  "fresh" value (the ledger risk row). Registry facts (identity status, term
  sequence, identity expiry) come from the enrollment store (migration 0018).
- **POLLING, not SSE (recorded DEFAULT):** this surface is a plain GET; the
  report imposes no live-SSE requirement on the edge-status resource, and
  polling keeps the surface inside the existing read-only API discipline.
  Revisit trigger: an operator workflow that needs push updates.
- **Adapter health passes through VERBATIM:** the P6.17
  `HealthSnapshot.json()` string is served untouched and the console renders
  it as-is (`SitesEdgeStatus.tsx` `adapter-health-verbatim` block) — the
  closed HEALTHY / DEGRADED_* / FAILING_* vocabulary is never prettified
  into healthy-looking synonyms.
- **Command-path state (P6.14):** closed values `DISABLED` /
  `TEST_ENV_ENABLED` / `NOT_REPORTED`. An out-of-vocabulary report renders
  `NOT_REPORTED` with the raw value carried in the note (fail-honest, the
  stateVocabulary rule) — never silently mapped onto a plausible state.
  `TEST_ENV_ENABLED` renders with its enablement expiry.

**Console.** New files (`Sites.tsx` itself is another wave's file — the
extension follows its patterns in NEW files, per the new-files-over-rewrites
rule):

- `api/edgeStatus.ts` — DTO mirrors + `fetchEdgeStatus` + the
  certificate-expiry treatment (`expired` / `rotation-due` / `valid` /
  `not-reported`; the P6.06/P6.17 7-day rotation window; text-borne tokens
  with pinned testids, the ValueType.tsx pattern).
- `workspaces/sitesEdgeStatus.ts` — loading (per-edge slots, per-slot
  verbatim errors — the Sites `viewError` pattern), mapping, and the LOCAL
  command-path request log (the `security/denialLog.ts` pattern: local
  diagnostics only, never sent to any server).
- `workspaces/SitesEdgeStatus.tsx` — the panel: connectivity last-seen,
  certificate-expiry treatment, verbatim adapter health, buffer depth, data
  freshness PROMINENT (own heading + testid; the freshness line reuses
  `components/ValueType`; the measured/estimated/stale/simulated treatments
  stay decided ONCE by the shared `valueTypeFor` treatment record — no new
  visual language), and the command-path block.
- **Wiring note (recorded for the console owner):** `App.tsx` / `Sites.tsx`
  are outside this wave's file ownership, so the panel is delivered as
  `SitesEdgeStatusSection` ready to embed in the Sites workspace (one
  component import + a `loadEdgeStatuses` call per site's edge identity).
  The tests pin the rendered contracts directly against the section.
- **THE COMMAND-PATH GUARANTEE:** the console NEVER enables the command
  path. Verified against `routes_edge.rs` and `openapi.rs`: no endpoint on
  the server surface can enable it, none is called, none is invented. The
  only affordance on an enabled/unknown path is "Record request (local
  console event only)", gated behind an explicit confirmation checkbox; on
  a DISABLED path the panel explicitly states there is no enable affordance
  and that enablement starts OFF this console. Every command-path display
  carries the proposed-vs-executed statement (a proposed setpoint is never
  an executed command; acknowledgement is NOT physical delivery, §14.1).
- **Confirmation (P6.25 step 4):** the request affordance implements the
  `ConfirmHighImpact` pattern exactly (labelled required checkbox inside a
  `<label>`, `aria-required`, inline with the statement it confirms,
  keyboard-operable). The recorded `HIGH_IMPACT_ACTIONS` list is a closed
  Phase-5 vocabulary this wave does not own, so the confirmation is
  implemented in the new file following the pattern rather than passing an
  unlisted actionId into the shared component (which would render its
  unknown-action alert). Recorded as a DEFAULT: extending
  `HIGH_IMPACT_ACTIONS` with a `command-path-enablement-request` entry is
  the reviewed change if the console owner prefers composition.

**Console tests** (`SitesEdgeStatus.test.tsx`, UNRUN): states distinguishable
(treatments + testids), gaps never healthy-looking, proposed never executed,
TEST_ENV_ENABLED always labelled TEST ENV ONLY with expiry, no enable
affordance wired to any endpoint (the mock client records only GETs; the
request log stays empty until the confirmation flow runs), confirmation
required (checkbox + disabled button + nothing recorded by render alone).

## 2. P6.31 — edge time synchronization and clock-quality monitoring

**The closed state vocabulary** (`clock_quality.rs::ClockQualityState`,
exactly, no fifth state):

| State | Meaning | Expiry checks trusted? | Verifier seam (`for_p6_15_verifier`) | Health rendering |
|---|---|---|---|---|
| `Unknown` | no observation yet — the construction state, never healthy-by-default (P0.09 §4) | NO | `ClockQuality::Unknown` (the verifier fails it closed — verified against `command_verify.rs`) | `DEGRADED_UNVERIFIED` |
| `Synchronized` | median offset within the configured tolerance | YES | `ClockQuality::Synchronized` | `HEALTHY` |
| `Degraded` | a sync observation landed but the offset is BEYOND the tolerance (§17.4 drift) | NO | `ClockQuality::Unsynchronized` | `DEGRADED_CLOCK_DRIFT` |
| `Unsynchronized` | source unreachable (explicit note, or quiet past the sync timeout) | NO | `ClockQuality::Unsynchronized` | `DEGRADED_CLOCK_DRIFT` |

**Verifier seam conformance (the recorded note):** `command_verify.rs`
(P6.15) declares `CommandVerifier::verify(..., clock_quality: ClockQuality)`
taking the CONTRACTS `flexcore_contracts::time_model::ClockQuality` (a
closed three-value enum: `Synchronized`/`Unsynchronized`/`Unknown`) and
fails the expiry check on anything but `Synchronized` — `Unknown` included.
`command_verify.rs` was NOT edited. The producer seam is
`ClockQualityMonitor::p6_15_clock_quality(now)` +
`ClockQualityState::for_p6_15_verifier()`, conforming to that exact
parameter type. The contracts vocabulary has no `Degraded` value, and a
degraded clock is by definition NOT synchronized-within-tolerance, so
`Degraded` maps to `ClockQuality::Unsynchronized` (a true statement); the
richer four-state vocabulary is preserved in health/console surfaces.

**The hold-on-drift rule (§17.4/§14.3/§17.5 rule 5, P4.08 fail-closed):**
expiry checks are trusted ONLY on `Synchronized`. `ClockQualityMonitor::
expiry_check_allowed(now)` is the named pre-check, and
`clock_quality::audited_expiry_check(monitor, audit, now)` records the hold
(`EDGE_CLOCK_HOLD_ON_DRIFT: ...`, the dedicated-name-leads-detail pattern on
the closed `EdgeAuditKind`, the recorded P6.14 limitation) before returning
the refusal. Through the real verifier the rejection is the P6.15 `expiry`
reason with its own audit event. Every state TRANSITION is audited
(`EDGE_CLOCK_QUALITY_CHANGED: ...`).

**Sync source + tolerance (recorded DEFAULT-labelled decisions):**

| Profile | Sync source | DEFAULT tolerance (max acceptable offset) | DEFAULT sync timeout |
|---|---|---|---|
| `lab` | a named, allowlisted reference from configuration (`SyncSourceConfig.source_reference`, mandatory, non-empty) | 500 ms | 60 000 ms |
| `pilot` | the site's existing time service, named + allowlisted | 100 ms | 30 000 ms |

The host clock is NEVER assumed correct; an unnamed source is a
construction-time refusal. Both values are DEFAULTS for composition, not
measurements; each deployment records its own (configuration
`SyncSourceConfig`), and a nonsense tolerance/timeout is a named
composition error.

**NO NTP CLIENT IS WRITTEN (recorded).** An NTP client would be a new
dependency AND a new D6 network path. Offset estimation uses only the
bounded time-sample pairs the platform already exchanges (`TimeSamplePair`,
the §10.3 event_time-vs-ingest_time honesty shape): the monitor keeps a
bounded window (default 8 pairs) and estimates the median offset; a pair
whose absolute offset exceeds one day is REFUSED as corrupt, never
estimated. **DEPLOYMENT REQUIREMENTS (recorded):** (1) the site network
must permit the allowlisted sync source — the D6 record
(`03_d6_boundary_review.md`, P6.03) must list it, or the edge runs
`Unsynchronized` and holds commands (never a host-clock fallback);
(2) the deployment wires the fetch of reference pairs from that source.

**Health seam (additive, the reviewed closed-vocabulary change):**
`health.rs` gains exactly ONE new `HealthReason::ClockDrift` variant
(token `CLOCK_DRIFT`) — no existing variant, derivation rule, or test was
modified; the clock quality enters health only through
`clock_quality.rs`'s `health_record`/`health_status` seam (the `assess`
derivation stays adapter-scoped). `Degraded`/`Unsynchronized` render
`DEGRADED_CLOCK_DRIFT` (data flows, flagged; the COMMAND path is what fails
closed, via the verifier seam — not ingest), `Unknown` renders
`DEGRADED_UNVERIFIED`, only `Synchronized` renders `HEALTHY`.

**Tests** (`clock_quality_p6_31.rs`, UNRUN): forced offset beyond tolerance
→ Degraded + verifier rejection (`expiry` reason, audited) + audited hold;
sync-source outage (explicit note and quiet-timeout paths) → Unsynchronized
→ same; recovery → Synchronized and the same command verifies again;
Unknown-before-first-observation fails the REAL P6.15 verifier closed;
tolerance boundary respected exactly (at-tolerance inside, +1 ms Degraded);
health rendering; configuration honesty.

## 3. P6.32 — edge decommissioning and unenrollment

**Server-side** (`routes_edge.rs`, ADDITIVE — `POST
/api/v1/edge/decommission`):

- Operator-initiated; RBAC via the recorded-drift pattern (the interim gate
  evaluates `asset.deactivate` — granted to PlatformAdmin and TenantAdmin
  ONLY — because a closed-enum `edge.decommission` action is outside this
  wave's file ownership; single swap point documented).
- **Step-up = the confirmation binding:** the payload MUST name the edge
  identity, tenant, site, actor, and confirmation time (a confirmation that
  cannot name what it confirms is refused, 422/403), and the named actor
  must MATCH the authenticated subject and the named tenant must MATCH the
  verified tenant (a proxy confirmation is a refusal + audited REJECTED
  journal event). Real server-verified step-up (WebAuthn/MFA) remains the
  §17.3 deployment requirement it already is Phase-5-wide (recorded).
- **Revocation (the 0018 append-only term model, NO storage-crate change
  needed):** the handler appends a NEW registry row at the next
  `enrollment_seq` with the closed status `REVOKED` (the registry lifecycle
  is expressed as inserts, migration 0018) through the EXISTING
  `EdgeEnrollmentStore` trait — entirely inside this wave's ownership.
  The closed registry vocabulary stays ACTIVE/REVOKED/EXPIRED;
  "DECOMMISSIONED" is the operator-facing word for a REVOKED identity at
  operator request (the journal reason text carries
  `DECOMMISSION_CONFIRMED` with all five named fields).
- **The CONFIRMATION RECORD is persisted** in the append-only enrollment
  journal (event type `REVOKED`, reason = the full confirmation text naming
  device/tenant/site/actor/confirmed_at/revoked_at).
- **Fail-closed matrix (extends the P6.06 matrix):** unauthenticated 401;
  role not granted 403; actor/tenant mismatch 403 (+ audited); unknown or
  cross-tenant device 404 (existence never leaks); site ≠ registry binding
  403 (+ audited); already REVOKED 409 (+ audited); no store composed 503.
- **Reconnect-after-decommission:** the REAL server logic already refuses
  everything — `resolve_edge_session_scope` resolves only an ACTIVE term
  (a REVOKED current term = no session scope), and re-enrollment of a
  REVOKED identity is refused by the enroll route's existing branch. Both
  refusals are audited. Verified at the store level in-module (real
  resolver + real fake store) and mirrored edge-side in
  `decommission_p6_32.rs` (the `FakeControlPlaneRegistry` test-local fake
  of the same matrix — the real-router test file in `apps/control-plane/
  tests/` is another wave's; recorded).

**Edge-side wipe** (`enrollment.rs`, ADDITIVE — `DecommissionWipe`,
`DecommissionConfirmation`, `DecommissionGapLedger`, `AuditSettlement`):

1. **Credentials/certificates:** the `FilesystemCredentialStore` record file
   is OVERWRITTEN with zeros (same byte length), synced, and DELETED —
   including the `.partial` atomic-write sibling — then the public-API
   post-condition (`load() == None`) is verified; a wipe that leaves a
   loadable record is a named error, never a fake success. Idempotent
   (absent targets are success).
2. **Buffered telemetry (P6.09):** drained through the `LocalBuffer` port,
   committed-forwarded (the P6.09 at-least-once commit point compacts the
   journal to empty), post-condition `len() == 0` verified, and the journal
   file overwritten-and-deleted via its path.
3. **Un-uploaded audit records (P6.18) — THE EXPLICIT RECORDED RULE:**
   **UPLOAD-THEN-WIPE** when the channel is up: drain → upload → IN-ORDER
   `commit_uploaded` → journal wipe; an upload failure prepends the batch
   back in order and wipes NOTHING (never a partial loss). When the channel
   is UNREACHABLE, the loss is recorded as a **GAP EVENT** — counted
   (`DecommissionGapLedger.gap_records_total`), audited
   (`EDGE_DECOMMISSION_AUDIT_GAP`), and LATCHED
   (`gap_latched`, never unlatched) — BEFORE the journal is wiped. **Never
   silently dropped.**
4. **Forensic-erasure honesty (recorded):** the overwrite-then-delete is
   filesystem-CALL-level erasure; guaranteed non-recovery against the
   OS/SSD wear-levelling layer is a DEPLOYMENT REQUIREMENT (protected
   volume / full-disk encryption at decommission per the site's OT boundary
   review). No new dependency is taken and none is simulated.
5. **Idempotence:** every wipe converges — a repeated decommission (or a
   crash between steps) succeeds wiping nothing; a repeated audit settle
   re-records a (zero-record) gap rather than hiding it.

**Tests** (`decommission_p6_32.rs`, UNRUN): reconnect-after-decommission
rejected + audited (the matrix at the registry level and edge-side wiped
credentials = nothing to reconnect with); post-wipe nothing recoverable
through the public API (credentials + `.partial`, telemetry buffer + journal
file, audit journal); the audit rule's three branches (upload-then-wipe,
upload-failure wipes nothing, gap-event counted/audited/latched); the
confirmation record naming all five fields; idempotence.

## 4. Gaps recorded for their owners (human-gate inputs)

| Gap | Owner | Note |
|---|---|---|
| Postgres wiring of `EdgeStatusSource` | edge-status forwarder / control-plane composition | the seam + default exist; a composed reporter (whatever transport carries edge health inbound) is per-deployment work. `AppState::with_edge_status` is the wiring point. |
| Interim authz actions (`telemetry.read` for status read, `asset.deactivate` for decommission) | `flexcore-authz` owner | the closed-enum variants (`edge.status.read`, `edge.decommission`) + their grant arms are the reviewed change; the swap points are single call sites in `routes_edge.rs`. |
| Console embedding of `SitesEdgeStatusSection` | console owner (`App.tsx` / `Sites.tsx` outside this wave's ownership) | one component import + one `loadEdgeStatuses` call per site's edge identity. |
| `HIGH_IMPACT_ACTIONS` extension for the request affordance | console owner | currently the confirmation follows the pattern in the new file (see §1); the recorded-list extension is the alternative. |
| Real server-verified step-up (WebAuthn/MFA) | deployment / P5.26 successor | the confirmation binding here is payload-level, matching the shipped Phase-5 step-up honesty. |
| Forensic-grade erasure | deployment (OT boundary) | see §3 item 4. |
| Sync-source reachability in the D6 record | P6.03 owner | the pilot allowlist must list the named time service (§2). |
| Clock-quality production wiring in `runner.rs` | runner owner | `runner.rs` is outside this wave's ownership; the composition feeds `observe_pair`/`note_source_unreachable` per cadence tick and passes `p6_15_clock_quality(now)` into the verifier. |

## 5. UNRUN status

Everything above is written, not built, not run: `cargo` was not invoked at
all (loop bans). The human build/test pass should run (minimum set):

```sh
cargo test -p flexcore-edge-agent --test clock_quality_p6_31
cargo test -p flexcore-edge-agent --test decommission_p6_32
cargo test -p flexcore-edge-agent          # the in-module clock/decommission units
cargo test -p flexcore-control-plane routes_edge   # the in-module seam tests
cargo clippy --workspace -- -D warnings
cd console && npx vitest run src/workspaces/SitesEdgeStatus.test.tsx src/api   # when the console toolchain is installed
```

Until then, no claim of a passing build or test exists anywhere in this
record.
