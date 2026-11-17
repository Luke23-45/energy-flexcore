<!-- P13.22 — Decision records + phase close (close/exit-gate record).
     CLOSE RECORD ONLY: this record reports the phase's written artifacts,
     verifies the P13-E-1/E-2/E-3 register state, and carries the
     cross-subtask consistency audit. It makes NO deployment, token,
     provisioning, notification, delivery, enrollment, or login claim —
     nothing in this phase has executed or connected; every evidence cell
     in every artifact it reports is EMPTY/UNRUN. The ledger ticks for
     P13.15–P13.22 are the ORCHESTRATOR's action (conventions §2.2) — no
     agent ticked a checkbox. -->
# P13.22 — Decision Records and Phase Close (close/exit-gate record)

| Field | Value |
|---|---|
| Subtask | P13.22 (decision records + close: the P13-E-1/E-2/E-3 register verification, per-batch decision records, the cross-subtask consistency audit, HANDOVER + CHECKLIST + this close record) |
| Wave / agent | Phase 13 wave 2, Agent B |
| Status | WRITTEN-UNRUN close: this record reports the phase's written artifacts, quotes the three permitted-tool outputs verbatim in §2, and carries the consistency audit in §3. It makes NO claim about any undeployed IdP, unvalidated token, unprovisioned SCIM object, unsent notification, undelivered webhook, unenrolled MFA factor, or unperformed login. |
| Sources | Program conventions (`../../01_PROGRAM_CONVENTIONS.md` §2 — ledger discipline and the close deliverables; §4 — agents never close E-decisions; §5 — human gates and §13.3 semantics; §6.4 — the never-granted set); the phase plan §2 (the P13.22 row), §4 (the three E-decision entries), §8 (the phase's acceptance criteria); the wave close-outs (`records/19`, `records/27`, `records/48`); the gap analysis §11 (the [S-1]..[S-172] register) |
| Files touched | `HANDOVER.md` (new), `CHECKLIST.md` (new), this record. The ledger, plan.md, context.md, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, and every other agent's files are untouched — no checkbox was ticked by this agent. |
| Audit scope | §3 covers every phase-13 artifact PRESENT on disk at close time (2026-11-14): `context.md`, `plan.md`, `ledger_phase_13.md`, `HANDOVER.md`, `CHECKLIST.md`, records 01–08, 19, 21–27, 41–48, and 61 (this record). Agent A's wave-2 records (41–48) were wait-checked for, landed during this close, read in full, and included — the audit ran AFTER the last of them (record 48) was present; the scope is stated here explicitly per the close instruction. |

## 1. The close position (expected end state)

Phase 13's implementation is complete: 22 of 22 subtasks implemented
(P13.01–P13.08 wave 1 Agent A; P13.09–P13.14 wave 1 Agent B; P13.15–P13.21
wave 2 Agent A; P13.22 wave 2 Agent B), every one with a decision record
under `records/`. **The ledger's P13.15–P13.22 ticks are the
ORCHESTRATOR's action**, applied after this close per program conventions
§2.2 ("a checkbox is ticked only as 'implemented, awaiting human
build/test' with a decision-record citation") — no agent ticks a
checkbox, and this record writes the EXPECTED end state, not an applied
one:

| Ledger row | Status at close | Expected citation (applied by the orchestrator) |
|---|---|---|
| P13.01–P13.14 | TICKED (by the orchestrator) — each citation verified to resolve to a file on disk (§3.1) | records 01–08, 21–26 (as already cited) |
| P13.15 | unticked at close | `records/41_p13_15_tenant_lifecycle.md` |
| P13.16 | unticked at close | `records/42_p13_16_session_management.md` |
| P13.17 | unticked at close | `records/43_p13_17_console_auth_hardening.md` |
| P13.18 | unticked at close | `records/44_p13_18_admin_workspace_completion.md` |
| P13.19 | unticked at close | `records/45_p13_19_identity_test_suite.md` |
| P13.20 | unticked at close | `records/46_p13_20_idp_lab_spec.md` |
| P13.21 | unticked at close | `records/47_p13_21_notification_audit_interplay.md` |
| P13.22 | unticked at close | this close record |

The tick text stays "implemented, awaiting human build/test" — never
"tested", "verified", "passed", or "works".

**E-decision register state at close (verified; no agent closed anything;
no register entry was missing, so none was added):**

| ID | Status | Register record (verified on disk) | DEFAULT (RECOMMENDATION) | Closer |
|---|---|---|---|---|
| P13-E-1 (identity provider) | OPEN | `records/01_p13_01_identity_architecture.md` §1 (registered at wave 1); the DEFAULT executed by records 02 and 46 with the dependence recorded (conventions §4), never closed | OIDC-first with Keycloak as the reference IdP and lab target [S-77]; commercial IdPs supported as per-tenant SSO connections; budget = human owner | human owner (budget/vendor sign-off) |
| P13-E-2 (notification channel providers) | OPEN | `records/23_p13_11_notification_pipeline.md` §4 (registered at wave 1); the email-first DEFAULT implemented as data (EMAIL at WARNING+, SMS/webhook off), never closed | email-first with provider-abstracted ports; SMS/webhook designed, vendor selection deferred; no channel is a dispatch path | human owner (vendor selection + budget) |
| P13-E-3 (MFA mechanism for the enforced step-up) | OPEN | `records/04_p13_04_step_up_mfa.md` §5 (registered at wave 1); the DEFAULT's SEAM implemented (the `StepUpVerifier` port; the shipped default verifier denies everything), never closed | IdP-delegated MFA primary, TOTP documented fallback; WebAuthn/passkeys noted as the forward path | human owner |

All three are mirrored in `plan.md` §4 and in `ledger_phase_13.md`
§Decision points as OPEN with their DEFAULTs — the register verification
the subtask calls for found no missing entry (each E-decision was already
registered by its wave-1 record exactly as the dispatch brief expected),
so no new register record was created in the 61–69 range; this table is
the verification of record. **No E-decision was closed by any agent**
(conventions §4).

## 2. Permitted-tool outputs (verbatim)

Run 2026-11-14, after `HANDOVER.md` and `CHECKLIST.md` were written and
after the §3 audit sweeps; this close record quotes the outputs and was
the last file written. The tools check the report, the agent harness, and
the workspace dependency graph — not phase records.

`python tools/verify_docs.py`:

```text
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0
```

`python tools/validate_agent.py`:

```text
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

`python -I tools/check_dependency_rule.py` (the character rendered as
`ù` below is the console mojibake of the tool's separator glyph in this
Git Bash session — the same rendering every prior phase-11/12/13
close-out records; quoted verbatim as required):

```text
OK ù dependency rule holds for 20 workspace members.
EXIT=0
```

All three exited 0. Per the honesty rules this states the tools' own
output about the report, the harness, and the workspace dependency graph
— not a claim about the unrun modules, undeployed designs, or
unperformed identity/integration acts this phase delivered. 20 workspace
members = no crate was added by Phase 13; all Rust code is modules in,
or test files of, existing members, so the checker's edge table is
untouched.

## 3. Consistency audit (performed LAST, over every artifact present at close)

The audit is an INSPECTION (grep/read-based), not a tool run; it makes no
claim beyond what was inspected. Agent A's wave-2 records (41–48) landed
during this close; they were wait-checked for, then read in full, and the
audit below includes them.

### 3.1 Ledger decision-record citations

- 22 subtask rows exist in `ledger_phase_13.md`; the 14 ticked rows
  (P13.01–P13.14) each carry a `(decision record: records/NN_*.md)`
  citation, and EVERY cited file resolves on disk (verified file by
  file: records 01–08 and 21–26 — 14 of 14 EXIST).
- The 8 unticked rows (P13.15–P13.22) name their evidence in prose and
  await the orchestrator's citations per §1. No checkbox was ticked by
  this agent anywhere.

### 3.2 Table pipe counts

Every markdown table in the phase directory (`context.md`, `plan.md`,
`ledger_phase_13.md`, `HANDOVER.md`, `CHECKLIST.md`, and all 24 records
in `records/` at close, this record included) was swept for inconsistent
pipe counts WITHIN each table (fenced code blocks excluded). **One
inconsistency found — see §3.7 finding 1** (record 43 §2; Agent A's
file, recorded not fixed). Zero inconsistencies in every other file,
this record included.

### 3.3 [S-n] citation resolution

The set of `[S-n]` citations across the entire phase tree
(`context.md`, `plan.md`, `ledger_phase_13.md`, `HANDOVER.md`,
`CHECKLIST.md`, all records) is exactly: S-44, S-45, S-65, S-69, S-70,
S-75, S-76, S-77, S-83, S-84, S-85, S-86, S-87, S-88. All fourteen
resolve to entries in the gap analysis §11 register (S-1..S-172 verified
against `00_GAP_ANALYSIS.md`; the largest cited is [S-88]) — **zero
out-of-range citations**. The register's UNVERIFIED items are never
cited as fact: [S-65] is cited only for the JetStream per-message
deletion caveat being UNVERIFIED-AT-A-DISTANCE (record 25 §4); [S-84]'s
`Deprecation`-header companion is recorded as a DRAFT, not an RFC
(record 22 §2); [S-85]'s numeric bounds are this phase's registration of
guidance, not the source's claimed numbers (record 22 §6).

### 3.4 Honesty sweep (no IdP/token/SCIM/notification/webhook/MFA/login claims)

Sweep over every markdown artifact in the phase directory (grep + read),
targeting positive capability claims and placeholders:

- **No IdP is deployed, no OIDC token is validated, no SCIM object is
  provisioned, no notification is sent, no webhook is delivered, no MFA
  factor is enrolled, no login is performed, no tenant is created or
  operated on, no session is created or revoked, no export is produced,
  no DSAR is exercised, no erasure runs, no image is pulled, no realm is
  imported** anywhere in any phase-13 artifact. Every execution-shaped
  statement is negated or labeled WRITTEN-UNRUN / UNRUN / EMPTY /
  UNSIGNED / SPECIFIED-NOT-IMPLEMENTED. The three permitted tools'
  outputs (§2) are the only executed programs in this phase.
- The P13.20 lab spec's one verification claim is exactly what its
  record scopes it to: the pinned Keycloak version's EXISTENCE on
  keycloak.org (26.8.1, released 2026-10-13) — the pull and every
  runtime behavior stay UNRUN (record 46 §6).
- Every evidence cell in the record 45 §3 SCIM scenario tables is EMPTY
  (UNRUN) — the correct pre-lab state (X-8); no result was invented.
- The `grep` sweeps for `TODO`/`FIXME`/`XXX`/`todo!`/`unimplemented!`
  and for un-negated claim verbs (deployed/sent/delivered/enrolled/
  provisioned/validated) return ZERO hits across the phase directory.
- No conformance or certification claim (OIDC, SCIM, SAML, SOC 2)
  exists: records 03 §5, 46 §5, and the plan §1 carry the explicit
  no-conformance wording; certification remains the human-gated process.
- No protection-grade or control-authority claim: ADR-010 holds — the
  notification ports are receipt-only, the webhook payloads are event
  notifications, the export is read-only, and no phase-13 artifact adds
  dispatch authority to any principal (per-record never-granted quotes;
  §3.5 below).
- §13.3 semantics are stated where a finite check could imply a
  guarantee (records 41 §6 item 3, 46 §5 — "a finite lab pass is a test
  outcome, never a guarantee").

### 3.5 The byte-identical invariants (verified by inspection; quoted)

1. **The frozen 14-state / 27-edge command lifecycle.**
   `git diff --numstat HEAD -- crates/flexcore-domain/src/command_state.rs
   crates/flexcore-domain/src/tenant_context.rs
   crates/flexcore-events/src/events.rs` returns NO output (0 added /
   0 removed for all three — byte-identical to HEAD). The file's own
   frozen-set statement, quoted verbatim from
   `crates/flexcore-domain/src/command_state.rs` (header comment,
   unchanged):

   > - **14 states:** the 8 forward states PROPOSED, VALIDATED, FEASIBLE,
   >   APPROVED, ISSUED, ACKNOWLEDGED, TRACKING, COMPLETED; and REJECTED,
   >   FAILED, EXPIRED, CANCELLED, SUPERSEDED, FALLBACK.
   > - **27 state-to-state transitions**, reproduced verbatim from the freeze
   >   table. No shortcut edges exist: there is no PROPOSED → ISSUED, no
   >   PROPOSED → FEASIBLE, no VALIDATED → APPROVED. Each of the three gates is
   >   its own state transition.

   The in-file tests pin the counts (`CommandState::ALL.len() == 14`;
   `FROZEN_EDGES: [(CommandState, CommandState); 27]`); the P13.06
   engine (`command_gates.rs`) contains no lifecycle type and performs no
   state change (record 06 §1) — the extension is from outside, as the
   plan §2.2 rule requires.
2. **The frozen 26-event vocabulary.** `crates/flexcore-events/src/events.rs`
   is byte-identical (same 0/0 numstat). Quoted verbatim:

   > pub const EVENT_COUNT: usize = 26;

   with the in-file test still asserting `assert_eq!(FROZEN_EVENT_NAMES.len(), 26);`.
   The P13.21 registry IMPORTS `FROZEN_EVENT_NAMES` (never re-lists it)
   and pins the length equality by test (record 47 §1); the P13.11 map
   (`notifications.rs`) is untouched — 0 diff lines — and the registry's
   four Notify rows agree with the map's four rows row-for-row
   (TelemetryQualityChanged / AssetAvailabilityChanged / PluginFailed /
   DispatchFailed; verified by reading both tables).
3. **The never-granted permission set.** Quoted verbatim, unchanged
   (conventions §6.4):

   > The never-granted permission set (`dispatch.authorize`,
   > `dispatch.issue`, `secret.read`, `network.any`) stays byte-identical

   Verified by inspection: the four names do not exist on the `Action`
   surface and the negative tests remain in place (`action.rs`:
   `assert_eq!(Action::parse("dispatch.authorize"), None)` and the three
   siblings; `phase13_policies.rs` re-asserts all four for every role;
   the P13.19 authz-matrix test file asserts the same at integration
   level). No grant, scope, template, mapping row, disposition, or
   catalog action in any phase-13 artifact names any of the four; no
   phase-13 module imports a command/dispatch type (records 21 §6, 23
   §2, 25 §5, 41 §3, 47 §4).
4. **The single-construction-path claims contract**
   (`crates/flexcore-domain/src/tenant_context.rs`) — byte-identical
   (same 0/0 numstat); the P13.02 resolver attests verification through
   `AuthenticatedClaims::from_verified` and never constructs claims
   otherwise (record 02 §2).

### 3.6 The wave-1 handoff checks (the close brief's item d)

| Handoff | Verification result |
|---|---|
| The webhook secret FORMAT is consumed (never redefined) in B-wave-1's artifacts | VERIFIED. Record 08 §4 is the single definition (`whsec_<43 unpadded base64url>`, 32 bytes, strict parse). Record 21 §2 carries the RESOLVED reconciliation table (the literal form NOT restated; `SECRET_MIN_BYTES = 32` equals the decoded length exactly); the code consumes DECODED BYTES ONLY (`WebhookSigningKey::from_bytes`) — the `whsec_` strings that appear in `webhook_signing.rs`, `webhook_service.rs`, and migration 0020 are all CITATIONS to record 08 §4 ("defined ONCE there and consumed here"), never a second parse rule or a redefinition; migration 0020 stores `secret_current_ref`/`secret_previous_ref` + fingerprints, never material, and the `WebhookSecretResolver` port resolves references at attempt time |
| The jsonwebtoken pin + compile ASSUMPTION is recorded | VERIFIED. `apps/control-plane/Cargo.toml` carries the additive pin `jsonwebtoken = "11.1.0"` with the recorded rationale comment; record 02 §4 documents the pin (11.1.0 published 2026-09-16; the 12.0.0 of 2026-11-13 deliberately not taken) and record 02 §5 item 1 registers the compile ASSUMPTION ("verified ONLY at the first human build"); record 19 §5 item 3 repeats it in the wave-1 close-out |
| The 0020 migration secret-reference resolution is documented | VERIFIED. Record 21 §2 documents the amendment (the initial draft's material-in-DB columns replaced by references + fingerprints; the resolver port added) and the migration's own comments state the rule ("NO SECRET MATERIAL and NO secret FORMAT … consumed by citation"); record 27 §2 (the wave-1 B close-out) records the same reconciliation as its one fixed CONFLICT; record 25 §1.A carries the corrected store shape into the privacy inventory |

### 3.7 Findings (recorded, not fixed — none is this agent's file to fix)

1. **Pipe-count inconsistency in record 43 §2 (UNFIXED — Agent A's
   file).** The `Enforced-protocol step-up flow` row of record 43's
   §2 table contains a literal `|` inside a cell
   (`idle → challenge-issued → verifying → verified | denied`), making
   that row 5 pipes against the table's 4. The cell text is readable and
   no fact is affected; the fix is escaping or rewording that one cell in
   `records/43_p13_17_console_auth_hardening.md` at the next legitimate
   edit of that record (the orchestrator's or Agent A's call). Every
   other table in the phase is consistent.
2. **The ledger's trailing status line is stale (UNFIXED — the
   orchestrator's file).** `ledger_phase_13.md` line 3 correctly reads
   "IN PROGRESS (wave 1 complete) — 14/22", but the file's last line
   still reads "Status: **NOT STARTED (0/22).**" (the planning-time
   footer). The tick discipline and this close are unaffected; the
   footer should read the 22/22 expected end state when the orchestrator
   applies the P13.15–P13.22 ticks.
3. **Cosmetic path-placeholder variance across the specified-not-
   implemented registers (no action required).** Record 41 §5 names the
   tenant-export endpoint as `GET /api/v1/tenants/{id}/export` while
   record 44 §3 names it `GET /api/v1/tenants/{tenantId}/export`; record
   42 §4 names `DELETE /api/v1/sessions/{id}` while record 44 §3 names
   `DELETE /api/v1/sessions/{sessionId}`. The same endpoints, same
   methods, same designed-not-built status — only the placeholder token
   differs, and record 44 §5 item 2 explicitly records that a later
   implementation may adjust paths. Recorded so the variance is visible,
   not silently inherited.
4. **Agent A's wave-2 records landed during this close** (wait-checked
   at three intervals; the last, record 48, was present before the audit
   sweeps ran) — the audit-scope wait-check the brief required was
   satisfied, and §3 states the scope explicitly.

### 3.8 The cross-record consistency checks (the wave-2 handoffs, closed)

| Check (source) | Result |
|---|---|
| The export mechanism is ONE (record 41 §7 note 3: "check that statement against record 24") | VERIFIED: record 41 §4 reuses the record-24 pack shape (one mechanism, two entry points — the audit export and the tenant export); no second pack format exists in any record |
| The P13.11 → P13.21 registration (record 23 §7 note 2; record 47 §6 note 2) | VERIFIED: `EVENT_NOTIFICATION_MAP` is byte-identical (4 rows); the P13.21 registry carries all 26 dispositions (4 Notify agreeing row-for-row, 22 AuditOnly with rationales); rows, never logic |
| The specified-not-implemented register spans records 41 §5 / 43 §4 / 44 §3 without a second register (record 48 §6 note 1) | VERIFIED: the same endpoints are named in each owning record; no second register was created; the only variance is §3.7 finding 3's placeholder tokens |
| The P13.19 test files extend exactly the named records (record 45 §6 note 2) | VERIFIED by reading the "Extends" column: 05 (authz), 04 (step-up), 42 (sessions), 21 (webhooks), 03 (SCIM mapping) — all resolve; no test redefines another record's contract |
| The P13.16 → record 41 D-3 cross-reference (record 42 §8 note 4) | VERIFIED: record 41 §3 D-3 cites record 42's `revoke_all_for_tenant`; both sides record it |
| The P5.26 UI-only flow is superseded by a consumer, not by record 04 (record 04 §9 note 1) | VERIFIED: `stepUpApproval.tsx` is NOT edited; record 43 §3 carries the coexistence discipline (the enforced flow's honesty note; retirement belongs to the future composition that wires a gated action) |

## 4. Deviations from the plan (all recorded, none silent)

1. **No separate P13-E register record was created in the 61–69 range.**
   The dispatch brief required verifying that P13-E-1/E-2/E-3 are
   registered OPEN with DEFAULTs and adding "any missing register entry"
   — the verification (§1 table) found all three registered OPEN by
   their wave-1 records, so nothing was missing and no new file was
   needed; the close record carries the verification of record instead
   (the Phase 11 close created a separate record only because P11-E-1
   had NO register record — a different situation).
2. **Record numbering: 61 for the close**, within the assigned 61–69
   range; 62–69 are unused, not reserved against anything.
3. **The audit-scope wait-check is recorded** (§3.7 finding 4): Agent A's
   records 41–48 landed while this close was being written; the
   instruction's wait-check-then-audit order was followed, and §3 covers
   every artifact present at close time including all of Agent A's
   outputs.
4. The permitted tools were run once, after `HANDOVER.md` and
   `CHECKLIST.md` were final and after the audit sweeps; this record was
   written last and quotes those outputs (§2 states this order honestly).

## 5. Handoff notes for the orchestrator

1. Apply the ledger ticks for P13.15–P13.22 after this close, citing
   records 41, 42, 43, 44, 45, 46, 47, and this record per §1
   (conventions §2.2/§2.3 exact-match method or Write/Edit); update the
   ledger's stale trailing status line (§3.7 finding 2) in the same
   pass. Expected end state: 22/22.
2. Optionally fix record 43 §2's one-cell pipe count (§3.7 finding 1) at
   the next legitimate edit of that record.
3. The phase gate is NOT MET as a runtime claim. The human-gate queue,
   in dependency order: the first human build/test of the Phase 0–10
   workspace (all compile ASSUMPTIONS, including the jsonwebtoken pin
   and the hand-written-crypto vector tests); the first console install
   (`npm test`); then the P13.20 lab runbook R-1..R-8 (the first real
   token, the first SCIM push, the record 45 §3 scenario cells); the
   operator tuning of the registered thresholds; and the human closers
   for P13-E-1/E-2/E-3 plus the D1/LR-062 jurisdiction decision and
   legal sign-offs (CHECKLIST §Human-gate-inputs has the full table).
4. The three E-decisions stay OPEN; their closers and closing evidence
   are named in §1's table. Phase 16 consumes P13.07/P13.10 and Phase 19
   consumes the audit-export/privacy records unchanged (plan §5) —
   records, not new copies.
5. The claim vocabulary for everything that follows (builds, lab runs,
   sign-offs): zero observed violations from a finite test set is a test
   outcome, never a guarantee (conventions §5, §13.3); no number of
   green lab runs converts any claim into a guarantee or a conformance
   statement.

Status: **PHASE 13 WRITTEN-UNRUN, 22/22 subtasks implemented** — the
phase's deliverables are complete as written artifacts; every execution
or connection act they describe is a human gate that has not happened and
is not scheduled. The phase close is complete per conventions §2.5
(HANDOVER, CHECKLIST, this close/exit-gate record).
