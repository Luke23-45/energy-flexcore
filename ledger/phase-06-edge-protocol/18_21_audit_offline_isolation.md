# P6.18 + P6.21 — Local audit buffer / offline behaviour, and multi-site multi-tenant isolation at the edge: IMPLEMENTED (awaiting human build/test)

Date: 2026-11-13. Subtasks: P6.18, P6.21. Owner roles: Rust engineer (P6.18),
security engineer (P6.21, per ledger).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-06-edge-protocol/ENTRY_NOTE.md`):** the code and tests in this
wave are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass executes
it. Nothing in this document claims a test passed, a build succeeded, or a
measurement was taken.

Files (complete list; nothing outside it was touched):

| File | Status |
|---|---|
| `crates/edge-agent/src/audit_buffer.rs` | NEW (P6.18, consumed by P6.21) |
| `crates/edge-agent/src/lib.rs` | EDITED (additive `pub mod audit_buffer;` only) |
| `crates/edge-agent/tests/audit_buffer_p6_18.rs` | NEW (P6.18) |
| `crates/edge-agent/tests/tenant_isolation_p6_21.rs` | NEW (P6.21, edge side) |
| `apps/control-plane/src/routes_edge.rs` | EDITED (additive P6.21 hardening: `resolve_edge_session_scope` + unit tests; no existing route modified) |
| `apps/control-plane/tests/api_edge_isolation_p6_21.rs` | NEW (P6.21, server side) |
| `ledger/phase-06-edge-protocol/ledger_phase_6.md` | EDITED (P6.18/P6.21 ticks only) |
| this file | NEW (decision record) |

## 1. P6.18 — the audit buffer (`crates/edge-agent/src/audit_buffer.rs`)

`FileAuditBuffer` is the durable edge-local audit buffer, mirroring the
P6.09 mechanics of `buffer_file::FileLocalBuffer` (patterns reused, not
reinvented):

| Ledger requirement | Implementation |
|---|---|
| Append-oriented, bounded, survives restarts | append-only journal `FLEXCORE-EDGE-AUDIT-JOURNAL` + version 0x01, length-prefixed CRC-32 records (the exact P6.09 framing discipline, std-only); capacity is a record bound given at open (`assert capacity > 0` — bounded by construction); appends are `sync_data`-flushed disk-first, then memory |
| Records closed vocabulary, own enum | `BufferedAuditKind` — a CLOSED vocabulary INSIDE the module: enrollment kinds (`EnrollmentCompleted/Rejected`, `CredentialExpiryWarning/Expired`, `RevocationUnchecked`), command kinds (`CommandPathEnabled/Disabled`, `CommandAccepted/Rejected`), health kinds (`SupervisionStartup`, `DegradedBufferingEntered/Exited`, `ForwardBatch`, `SourcePollFailed`, `BufferFull`), rejection/isolation kinds (`ScopeDenied`), and the buffer's own integrity kinds (`AuditBufferOverflow`, `AuditBufferCorruptTail`). Stable `EDGE_*` names follow the `enrollment.rs` naming style. `EdgeAuditKind` (ports.rs) is NOT modified — the recorded P6.14 limitation stands, unchanged and un-fixed |
| Upload on reconnect, in order, at-least-once | `drain` removes from memory ONLY (front-first, lowest persisted sequence); the journal keeps records until `commit_uploaded` — the mirror of the P6.09 `commit_forwarded` commit point — compacts to the still-pending set (temp file + rename). A crash in the drain→upload→commit window re-delivers after restart (§15.4 at-least-once). Sequences are assigned at append and PERSIST in the journal, so upload order is the same global order across restarts. `commit_uploaded` is fail-closed IN-ORDER: a batch that does not start at the journal's front is refused with `AuditUploadError::OutOfOrderCommit` and removes NOTHING — an un-uploaded record is never removed out of order and never overwritten in place |
| Overflow: never silently drop | the recorded P6.09 pattern: audited drop-of-OLDEST — hard counter (`dropped_oldest`), LATCHED health flag (`has_overflowed`), loud log, and the reconciliation identity `appended_total == committed + pending + dropped` within one open. The drop is REPORTED loss, never silent, never an in-place overwrite |
| Corrupt-tail recovery | truncate-and-audit at the first malformed frame (bad length / CRC mismatch / undecodable payload / stray bytes), counted (`corrupt_tail_truncated`), never guessed past; a wrong magic/version is a hard open error |
| Event-count reconciliation | `appended_total()` (includes the replayed journal at open) vs `pending_len()` / `dropped_oldest()` / committed count — asserted end-to-end in `tests/audit_buffer_p6_18.rs` |
| Encryption at rest (§18.3) | **NOT implemented — DEPLOYMENT REQUIREMENT (recorded twice: `08_09_ingest_buffer.md` §6 and here).** Until closed with a P6.02-audited crypto dependency, the audit journal holds records in the clear on the edge host; the deployment profile (P6.20/D6) must treat the audit-journal path as in-scope for OS-level site protection (protected volume / filesystem permissions / full-disk encryption per the site's OT boundary review). No new dependencies were permitted or added |

The buffer implements the edge `AuditSink` port (`record(&self, ...)`,
interior-mutable via `Mutex`), so the composition root's planned swap
(`InMemoryAuditSink` → durable buffer, itemised in the P6.14 decision doc
§7) is mechanical. The `EdgeAuditKind` → `BufferedAuditKind` mapping never
rewrites a detail string: command-path events keep their dedicated
`EDGE_COMMAND_*` name leading the detail (the recorded closed-`EdgeAuditKind`
limitation — the greppable identity survives, and adding dedicated variants
to `EdgeAuditKind` remains the recorded follow-up, NOT done here).

Audited-counter limitation (recorded, same as P6.09): the counters are
in-memory per open; a restart loses the counts (each loss event was audited
and logged at the moment it happened). Persisting counters is a P6.17
candidate. There is deliberately NO age-based drop for audit records — audit
evidence retention is a deployment `RetentionPolicy` decision (P0.16 §4),
not a period in code (decision D-2 below).

## 2. The OFFLINE BEHAVIOUR STATEMENT (ledger P6.18 step 2 — in writing)

1. **The local site EMS remains authoritative for fast safety behaviour.**
   FlexCore is a supervisory orchestration layer (AGENTS.md rule 3): it does
   not perform protection-grade, inner-loop, or sub-cycle control, and the
   edge agent never replaces the site EMS's protective behaviour.
2. **While disconnected, the edge issues NO new commands — the test-env
   path included.** Cited mechanism: the P6.14 gate is read-time-checked
   (`CommandPathRuntime::enablement_at` re-derives the state on every
   receive, so enablement can never be held stale), and the channel that
   carries response-direction frames is the OUTBOUND mTLS session (P6.07)
   — DOWN while disconnected, so nothing can arrive, be admitted, or be
   queued for later issuance. A refused command is retained nowhere: the
   receiver has no queue field, and nothing is forwarded on reconnect (the
   recorded P6.18 risk — "helpfully issuing queued commands after their
   validity expired" — is structurally excluded: there is no command queue).
3. **Telemetry buffering while offline follows the P6.09 design**
   (`FileLocalBuffer`): bounded, audited drop-of-oldest, in-order
   store-and-forward, at-least-once commit semantics.
4. **A central outage does NOT disable site-local protective controls and
   does NOT force any unsafe device state (§21.2).** The edge agent has NO
   field-write authority at all in v0.4.0 (read-only / test-lab by
   default, §2.1); the command path is disabled by default (P6.14) and the
   site EMS's protective behaviour is entirely local to the site.

## 3. P6.18 — tests (`crates/edge-agent/tests/audit_buffer_p6_18.rs`) — ALL UNRUN

| Ledger test requirement | Test |
|---|---|
| Central outage during a command flow — events accumulate in order | `central_outage_during_a_command_flow_events_accumulate_in_order` — the default-gate receiver + the degraded runner record gate state, `EDGE_COMMAND_REJECTED` (P4-pattern denial), degraded entry, and poll failure into the durable buffer, strictly ordered |
| Edge restart with a non-empty audit buffer | `edge_restart_with_a_non_empty_audit_buffer_uploads_in_order_after_reconnect` — records survive the restart, upload in persisted-sequence order, committed records never redeliver |
| Audit-buffer overflow — explicit reported loss, never silent | `audit_buffer_overflow_is_explicitly_reported_never_silent` — hard counter, latched flag, reconciliation identity, oldest dropped / order preserved |
| In-order upload guarantee | `upload_is_in_order_only_and_never_skips_an_un_uploaded_predecessor` — a commit that would skip un-uploaded seq 3–4 is refused and removes nothing; retry order 3→4→5→6 |
| Event-count reconciliation before/after reconnect | `outage_window_event_count_reconciles_before_and_after_reconnect` — the ledger's 30-minute outage expressed as a BOUNDED SYNTHETIC window (30 one-minute ticks on a fixed clock; every tick audited; dense sequences 1..=31; timestamps advance with the window and are never re-stamped; zero loss within capacity; commit → restart holds nothing) |
| Command rejections durably audited across restart | `command_rejections_during_an_outage_are_durably_audited_across_restart` |
| Audit-or-refuse honesty | `a_failed_audit_write_is_a_named_error_never_a_silent_skip` |

**THE REAL 30-MINUTE LAB RUN IS A HUMAN GATE INPUT — RECORDED AS UNRUN, with
an empty result:** no lab, no fault injector, and no cargo execution exist
in this loop. See §7.

## 4. P6.21 — edge-side isolation (`crates/edge-agent/tests/tenant_isolation_p6_21.rs`) — ALL UNRUN

The edge's scope source is EXCLUSIVELY its BOUND IDENTITY (the P6.06
`DeviceBinding` — one tenant, one approved site, §16.2). The test defines
`EdgeScopeGuard` built ONLY from public API (`DeviceBinding` +
`FileAuditBuffer`) with deliberately NO parameter by which a request could
carry, override, or widen the scope — the §16.2 warning is enforced by
construction (a request-carried scope claim is impossible to inject; test
`scoping_comes_from_the_bound_identity_never_a_request_carried_field`
proves the claim-irrelevance and the incomplete-binding fail-closed
default).

| Ledger step-1 case | Test / mechanism | Audited as |
|---|---|---|
| Edge A requests tenant B's telemetry → fail | `edge_a_requesting_tenant_b_telemetry_is_denied_and_audited` (the bound scope itself passes — the guard is a scope check, not a blanket denial) | `EDGE_SCOPE_DENIED` record: actor, object, stamped timestamp, requested vs bound scope in the detail |
| Edge A presents a command for site B → fail (P6.14 gate surface, read-only — `command_path.rs` NOT edited; the denial path is constructed from the public API) | `edge_a_presenting_a_command_for_site_b_is_refused_by_the_p6_14_gate_surface` — (a) the DEFAULT gate refuses outright (`command_path_disabled`); (b) even a gate ENABLED for site A (all four P6.14 conditions, signed, time-bounded) refuses the site-B adapter (`adapter_not_allowlisted` — the per-site allowlist is the per-command check); nothing is admitted (`EDGE_COMMAND_ACCEPTED` absent) | two `EDGE_COMMAND_REJECTED` P4-pattern records (`command=… asset=… code=… at=… reference=…`) + one explicit `EDGE_SCOPE_DENIED` (actor/object/timestamp) |
| Edge A enumerates assets outside its scope → fail | `edge_a_enumerating_assets_outside_its_scope_is_denied_and_audited` — wrong tenant AND wrong-site-within-tenant both denied; the bound site is allowed | `EDGE_SCOPE_DENIED` per denial |
| Denials audited (actor, object, timestamp) | every guard denial records `BufferedAuditKind::ScopeDenied` with non-empty actor + object + the stamped `recorded_at`, through the P6.18 durable buffer | `isolation_denials_are_durably_audited_and_upload_in_order_after_reconnect` — the denial evidence survives a restart and uploads in order (the audit trail survives the exact window an incident investigation needs) |

## 5. P6.21 — server-side enforcement (defence in depth; `apps/control-plane/`)

**Never rely on the edge's goodwill.** The server-side rule: every
edge-session request is scoped by the SERVER to the edge's enrolled identity
— tenant + site from the enrollment store (migration 0018, the authoritative
binding), never from any request-carried field.

What existed already (proven by tests, not changed): the enrollment route
scopes the binding against the provisioning token's server-side approved
scope (a request body claiming a foreign tenant/site is 403 — the token
scope is the server's, not the request's), checks site approval in the
tenant, and journals refusals (migration 0018 append-only journal). The
`api_edge.rs` suite covers this; the new suite re-proves it from the
isolation angle by driving the router AS A MALICIOUS CLIENT.

Minimal additive hardening (this wave): `routes_edge::resolve_edge_session_
scope(store, ctx, device_id)` — the single hardening point future edge-
session surfaces (telemetry upload, command response) MUST compose. The
tenant comes from the authenticated context and the site from the store
row; there is NO parameter by which a request could supply or widen any
part of the scope. Fail-closed: an identity that does not resolve to an
ACTIVE term in the caller's tenant scope has NO scope — a cross-tenant
identity is INVISIBLE (`UnknownIdentity`, P0.16 rule 5), a REVOKED/EXPIRED
identity has no scope (`IdentityNotActive`). No existing route was
modified.

`apps/control-plane/tests/api_edge_isolation_p6_21.rs` (ALL UNRUN — the
test drives the SERVER endpoints directly, as a malicious client would):

| Case | Test | Result asserted |
|---|---|---|
| Cross-tenant binding | `a_cross_tenant_binding_is_403_and_never_journaled_as_an_enrollment` | 403, no journal event |
| Foreign site within the tenant | `a_foreign_site_within_the_token_tenant_is_403_and_audited` | 403 + journaled REJECTED with actor, object (device), tenant, timestamp, correlation |
| Duplicate identity | `a_duplicate_active_identity_is_409_and_the_attempt_is_audited` | 409 `EDGE_IDENTITY_EXISTS` + journaled REJECTED |
| Revoked identity | `a_revoked_identity_refuses_the_session_scope_and_the_re_enrollment` | 403 + journaled REJECTED, AND no session scope (`IdentityNotActive`) |
| Scope from the store, not the request | `the_server_derives_the_session_scope_from_the_enrollment_store` | resolver takes no request scope field; scope == store row |
| Cross-tenant invisibility | `a_cross_tenant_identity_is_invisible_to_the_server_side_scope` | tenant A cannot resolve tenant B's edge; tenant B can |
| Unenrolled identity | `an_unenrolled_identity_has_no_session_scope` | `UnknownIdentity` — fail closed |

## 6. The blast-radius note (ledger P6.21 step 4)

**What a compromised edge CAN reach:**

- Its OWN bound identity's site telemetry: the read path over its own
  allowlisted EMS adapter (P6.08/P6.12), within the approved asset scope of
  its single (tenant, site) binding (§16.2).
- Its OWN local state: its telemetry buffer (P6.09), its audit buffer
  (P6.18), its credential store (P6.06) — all local to the edge host.
- Its own allowlisted command path WITHIN the P6.14 gate + P6.17 policy:
  and only in a test environment, only with the four-condition signed gate,
  only within the time bound, only to allowlisted adapters, only as a
  gate+policy ADMISSION (P6.15 verification and P6.16 forwarding do not
  exist yet — a receipt is not an issuance, §14.1).

**What a compromised edge CANNOT reach:**

- Any other tenant or site: the enrollment registry binds one identity to
  ONE (tenant, site); the server scopes sessions to that binding
  (`resolve_edge_session_scope`); a cross-tenant identity is invisible to
  the resolver, and the edge-side scope guard accepts no request-carried
  scope.
- RLS-protected central tables beyond its scope: migration 0018 puts RLS
  ENABLE + FORCE with the shared `tenant_isolation` policy on
  `edge_device_registry` and `edge_enrollment_journal`, grants
  `flexcore_app` INSERT+SELECT only, and appends UPDATE/DELETE guards —
  see the RLS note below.
- Any other edge's identity: no two edges share an identity (the 0018
  partial unique index on `(device_id, tenant_id) WHERE status = 'ACTIVE'`),
  and a single-use provisioning token cannot mint a second identity.
- Plugin surfaces: there is NO plugin host in Phase 6 (WASM host is
  v0.5.0 / Stage 7); nothing to escape into (recorded honestly — also see
  P6.23's scope note).

## 7. RLS / app-role note, and the human gate inputs (all with EMPTY results)

**The §16.2/[R25] RLS warning (recorded):** the APPLICATION ROLE must not
own tenant tables or bypass RLS. The existing control is migration 0018
(`crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql`):
`ALTER TABLE … ENABLE ROW LEVEL SECURITY` + `FORCE ROW LEVEL SECURITY` with
the shared `tenant_isolation` policy (owner `flexcore_migrator`, runtime
`flexcore_app` with INSERT+SELECT only, plus append-only UPDATE/DELETE
triggers that refuse unconditionally — including for the owner). What is
NOT verified here, and cannot be in this loop:

| # | Human gate input | Status / result |
|---|---|---|
| H1 | **The real 30-minute central-outage lab run** (ledger P6.18 verification: kill the channel, run the command/telemetry flow, reconcile event counts before/after reconnect) | **UNRUN — empty result.** The ledger scenario is expressed in `tests/audit_buffer_p6_18.rs` as a bounded synthetic window only. A human must run the lab drill and record the reconciliation |
| H2 | **The modified-edge-client lab run** (ledger P6.21 acceptance: test against a maliciously modified edge client) | **UNRUN — empty result.** CANNOT run in this loop (no lab, no build). The new server-side suite covers the same attack surface from the protocol side (hostile requests driven at the real router), but the acceptance criterion names a modified client binary and remains open |
| H3 | **The app-role ownership review** (verify `flexcore_app` owns no tenant tables and holds no RLS-bypassing role attribute across ALL migrations, not only 0018) | **UNRUN — empty result.** Recorded as the standing §16.2/[R25] gate; owner: security engineer |

All build/test verification (cargo fmt/clippy/test on the new files) is
itself the standing human build/test gate recorded by every Phase 6 wave —
UNRUN here.

## 8. Decisions (DEFAULT-labelled where recommended, not decided)

| # | Decision | Status |
|---|---|---|
| D-1 | The audit buffer reuses the P6.09 journal mechanics (framing, recovery, compaction, overflow pattern) rather than a new design; it implements the `AuditSink` port so composition swaps the in-memory sink mechanically | DEFAULT |
| D-2 | NO age-based drop for audit records (evidence retention is a deployment `RetentionPolicy` decision, P0.16 §4); the record bound is the only automatic limit | DEFAULT |
| D-3 | `BufferedAuditKind` lives INSIDE `audit_buffer.rs`; `EdgeAuditKind` (ports.rs) is untouched and its recorded closed-vocabulary limitation stands | RECORDED constraint |
| D-4 | Upload commit is fail-closed IN-ORDER (`OutOfOrderCommit` refuses, removes nothing) — stronger than a best-effort prefix commit, because the ledger's requirement is absolute ("no record uploaded before its predecessors"; "never overwrite an un-uploaded record") | DEFAULT |
| D-5 | The `AuditSink` composition maps runner events with actor `edge-agent` / object = the kind name; dedicated actor+object records (P6.21 denials) use `record_event` explicitly; details are never rewritten | DEFAULT |
| D-6 | The edge-side isolation guard is TEST-side (`EdgeScopeGuard` in the test file) built from public API; no new edge port was invented (the task's rule: a needed port would live in `audit_buffer.rs` — none was needed) | DEFAULT |
| D-7 | Server-side hardening is exactly one additive resolver (`resolve_edge_session_scope`) + tests; no existing route was modified (v0.4.0's only server-facing edge endpoint is enrollment, which already enforces the store-side scope) | DEFAULT |
| D-8 | Encryption at rest for the audit journal: NOT implemented; DEPLOYMENT REQUIREMENT (OS-level protection until a P6.02-audited crypto dependency closes it) | OPEN ITEM (honest) |

## 9. Verification status (honest)

- `python tools/verify_docs.py`, `python tools/validate_agent.py`,
  `python -I tools/check_dependency_rule.py` — run at wave end; their
  ACTUAL output is recorded in the agent handover note. They check
  documentation and dependency direction, not Rust compilation.
- `cargo build`, `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo test -p flexcore-edge-agent`,
  `cargo test -p flexcore-control-plane` — **NOT RUN (banned this loop)**.
  Status: UNRUN — the human build/test pass owns them.
- The `simulated-test` fixture and all named approvers/tokens in tests are
  TEST-ONLY values; no key material, no real endpoint, no secret appears
  in any test or audit detail (§17.3).
