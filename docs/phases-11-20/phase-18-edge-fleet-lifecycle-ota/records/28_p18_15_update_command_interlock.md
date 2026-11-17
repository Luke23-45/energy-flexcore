# Record 28 — P18.15: The update-safety interplay with the command lifecycle (the interlock)

Date: 2026-11-17. Subtask: P18.15 (`ledger_phase_18.md` §P18.15). Status:
**IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (WRITTEN-UNRUN; conventions §2.2).
**No command exists; no update has ever been applied; no fleet exists — no
command and no update interact on hardware anywhere.**

| Field | Value |
|---|---|
| Files touched | `crates/edge-agent/src/update_command_interlock.rs` (NEW); `crates/edge-agent/tests/update_command_interlock_p18_15.rs` (NEW); `crates/edge-agent/src/lib.rs` (ONE additive module entry) |
| Consumed by citation, never redefined | `crate::command_path::FROZEN_COMMAND_STATE_NAMES` (the frozen 14-state vocabulary; `flexcore_domain::command_state` is the normative source the edge cannot depend on — the P6.05 rule); `crate::conformance::{ConformanceHarness, ConformanceReport}` (P6.13/P6.19); `crate::update_session::UpdateSessionStateMachine::begin_download` (the wave-1 recorded rule, UNCHANGED); report §5.6 (cited, never rewritten) |
| Ledger line | "Updates BLOCKED while commands are ISSUED/TRACKING on the device; drain-before-update (report §5.6 discipline); post-update conformance re-check (edge conformance module interplay)" — all three legs delivered below |

## 1. Leg 1 — the blocking rule, made REAL (the wave-1 citation is now wired)

The wave-1 flow (record 04 §1) consumed this rule BY CITATION:
`UpdateSessionStateMachine::begin_download(issued_or_tracking_commands, …)`
refuses with `UpdateSessionError::CommandsInFlight` when the caller-supplied
count is non-zero. That parameter is a SEAM, not the rule — nothing computed
it from the command path, and an unknown device state was silently equal to
"zero in flight". This record wires the REAL rule:

`UpdateCommandInterlock::authorize_update_attempt()` — the device-state
check consumed FROM the command path, through the
`DeviceCommandStateSource` port (production wiring binds it to the P6.16
engine's recorded §14.1 states — `CommandForwardingEngine::lifecycle_state_of`
over the ids the composition issued; itemized in §6). Evaluation order
(recorded, deterministic):

1. the post-update conformance re-check of the LATEST activation
   (`Outstanding` → refuse; `Failed` → refuse, terminal until a recorded
   override — §3);
2. the device command states read through the source — a source error OR a
   state name outside the frozen 14-state vocabulary refuses
   `DeviceStateUnknown` (FAIL-CLOSED: an unobservable command state is
   NEVER read as "no commands in flight");
3. any command in a NON-TERMINAL frozen §14.1 state refuses
   `CommandsInFlight { issued_or_tracking: <count> }`;
4. otherwise authorized with `in_flight_count == 0` — the exact value the
   composition hands to the UNCHANGED wave-1 `begin_download`, so the two
   rules cannot disagree in the unsafe direction.

Every refusal is audited (`EDGE_UPDATE_INTERLOCK_BLOCKED`, stable code); the
authorization is audited (`EDGE_UPDATE_INTERLOCK_AUTHORIZED`). The
interlock holds NO update authority: it is ONE gate among the P18.13
attestation gate, the P18.06 orchestrator, and the untouched P9.34
verification-before-any-write surface.

**The honest delta vs the recorded citation (record 04 §1), stated here
because record 04 is not editable by this agent:** the RECORDED citation
blocks ISSUED/TRACKING; the REAL rule blocks EVERY NON-TERMINAL frozen
state — a STRICT SUPERSET that additionally blocks ACKNOWLEDGED, FAILED,
and EXPIRED (the frozen table's non-terminal states the narrower citation
misses: FAILED and EXPIRED exit only via FALLBACK, frozen edges 26–27, so
a command in either is still in flight), and any pre-issue state observed
on a device (itself a defect; refusing the update while it stands is the
fail-closed reading). This is a STRENGTHENING, never a relaxation; the
refusal keeps the recorded stable name `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT`
so the wave-1 tests and P18.14's concurrent-command tests cite ONE code.
The drain target set is exactly the frozen five terminal states
(`FROZEN_TERMINAL_COMMAND_STATE_NAMES`: COMPLETED, REJECTED, CANCELLED,
SUPERSEDED, FALLBACK) — cited from the frozen table, never forked.

## 2. Leg 2 — drain-before-update (report §5.6 discipline, cited, never rewritten)

Report §5.6 (cited verbatim in the module header): "For command-producing
plugins, activation or upgrade must not occur in the middle of an active
dispatch transaction. Drain the plugin, stop accepting new work, finish or
cancel pending operations, record the version change, and then replace
it." The device analog is `UpdateCommandInterlock::{begin_drain,
poll_drain}` over the `DrainPhase` vocabulary (`NotDraining` / `Draining`
/ `Drained`):

- a drain cannot START or COMPLETE while the device command state is
  unobservable (`InterlockError::DeviceStateUnknown`; the phase is
  retained, the refusal audited) — fail-closed;
- `allows_new_command_work()` is the §5.6 "stop accepting new work" read
  for the composition (the command path's own P6.14 semantics are
  UNTOUCHED — this module is additive);
- the procedure is OBSERVER-ONLY toward the command lifecycle: it NEVER
  transitions a command state. The "finish or cancel pending operations"
  leg is the OPERATOR's action through the normal frozen lifecycle (e.g.
  the frozen ISSUED → CANCELLED edge, transition 17); an update path that
  wrote command states would bypass the command lifecycle, which plan §4
  forbids. A written-unrun test pins that begin/poll drain leaves the
  command path's record unchanged;
- `Drained` alone does not authorize: the state is re-read AT AUTHORIZE
  TIME (`authorize_update_attempt`), so the drain and the blocking rule
  cannot diverge.

## 3. Leg 3 — the post-update conformance re-check (the P6.13 interplay)

After an update session reaches `UpdateProgressState::Active` (the
terminal success state), the composition records the activation
(`record_update_activated`) and the re-check becomes OUTSTANDING: further
update authorization is REFUSED until a CLEAN §19.2(6) report is recorded.
The composition runs the EXISTING P6.13 harness
(`ConformanceHarness::run_fault_injection_set`) and hands the report to
`record_conformance_report` — `ConformanceReport::is_clean()` and
`contract_violations()` are CONSUMED, never reimplemented:

- a CLEAN report → `RecheckPhase::Satisfied` (authorization unblocked);
- a VIOLATING report → `RecheckPhase::Failed` — TERMINAL for that attempt;
  re-running the harness to fish for a pass is refused; clearing requires
  a RECORDED OVERRIDE (`RecheckOverride`: named actor + authority
  reference — the P9.34 `RollbackOverride` pattern, cited). The override
  re-OPENS the re-check as OUTSTANDING; it never grants a pass;
- each new activation supersedes the previous attempt's re-check (a fresh
  re-check per update).

Claim boundary (the P6.19 boundary, verbatim): a clean re-check proves
ONLY the §19.2(6) edge read-path contract — it is NOT protocol
conformance, NOT certification, and is never quoted as either.

## 4. The exact surface for P18.14's concurrent-command tests (the blocking-order handoff, landed early)

| Item | Value |
|---|---|
| Module | `crates/edge-agent/src/update_command_interlock.rs` (`pub mod update_command_interlock`) |
| Gate | `UpdateCommandInterlock::authorize_update_attempt(&self) -> Result<UpdateAttemptAuthorization, UpdateBlockRefusal>` |
| Authorization | `UpdateAttemptAuthorization { in_flight_count: usize /* always 0 on Ok */, at_unix_millis: i64, scope_note: String }` |
| Refusals | `UpdateBlockRefusal::{DeviceStateUnknown, CommandsInFlight { issued_or_tracking: usize }, ConformanceRecheckOutstanding { attempt_id }, ConformanceRecheckFailed { attempt_id, violations }}` with `name()`: `UPDATE_INTERLOCK_STATE_UNKNOWN` / `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` / `UPDATE_CONFORMANCE_RECHECK_OUTSTANDING` / `UPDATE_CONFORMANCE_RECHECK_FAILED` |
| Two-step composition | `let auth = interlock.authorize_update_attempt()?; session.begin_download(auth.in_flight_count, now)?;` — the P18.04 session rule is fed the interlock verdict verbatim |
| State source | `DeviceCommandStateSource::device_command_states() -> Result<Vec<DeviceCommandObservation>, InterlockStateError>`; `DeviceCommandObservation { command_id, state_name }` validated against `FROZEN_COMMAND_STATE_NAMES`; the in-memory `InMemoryCommandStateSource` is the TEST DOUBLE (`reporting(Vec<…>)`, `set_observations(…)`, `set_unobservable(reason)`) |
| Drain | `begin_drain()` / `poll_drain()` / `drain_phase()` / `allows_new_command_work()`; `DrainPhase::{NotDraining, Draining { started_at_unix_millis, commands_at_start }, Drained { completed_at_unix_millis }}` |
| Re-check | `record_update_activated(attempt_id)` / `record_conformance_report(&ConformanceReport)` / `clear_conformance_failure(&RecheckOverride { actor, authority_reference })` / `recheck_phase()` / `recheck_attempt_id()`; `RecheckPhase::{NotRequired, Outstanding, Satisfied, Failed { violations }}` |
| Terminal set | `FROZEN_TERMINAL_COMMAND_STATE_NAMES = ["COMPLETED","REJECTED","CANCELLED","SUPERSEDED","FALLBACK"]` |
| Audit events | `EDGE_UPDATE_INTERLOCK_BLOCKED` / `EDGE_UPDATE_INTERLOCK_AUTHORIZED` / `EDGE_UPDATE_DRAIN_STARTED` / `EDGE_UPDATE_DRAIN_OBSERVED_IN_FLIGHT` / `EDGE_UPDATE_DRAIN_COMPLETE` / `EDGE_UPDATE_ACTIVATED_RECHECK_OUTSTANDING` / `EDGE_UPDATE_CONFORMANCE_RECHECK_PASSED` / `EDGE_UPDATE_CONFORMANCE_RECHECK_FAILED` / `EDGE_UPDATE_CONFORMANCE_FAILURE_CLEARED` (dedicated-name-leads-detail on the closed `EdgeAuditKind` vocabulary, kind `EDGE_STARTUP`) |
| Statement pin | `INTERLOCK_STATEMENT` (pinned by tests in both the module and the integration file) |

Test files: in-module tests (blocking battery, drain battery, re-check
battery, statement pins) + the integration file
`crates/edge-agent/tests/update_command_interlock_p18_15.rs` (the full
blocking/drain/re-check walks through the PUBLIC surfaces, including the
real-P6.13-harness re-check test). ALL WRITTEN-UNRUN — execution is the
human gate (loop ban 3).

## 5. Honesty review of the wave-1 records (01–08, 21–27) — findings

Verdict first: **no honesty violation of the not-produced / not-applied /
no-fleet class was found in any wave-1 record.** Every record carries the
required statements; the ledger ticks (P18.01–P18.13) each carry a
decision-record citation (conventions §2.2) and P18.14–P18.16 are
unticked; the kernel policy and the never-granted set are verified below.
Three findings, one of them build-blocking:

**F-1 (SIGNIFICANT — build-blocking defect behind a record claim that is
not present in the file).** Record 05 §2 and record 08 §1 claim the
visibility edit `pub(crate) fn sha256` → `pub fn sha256` and
`pub(crate) fn hmac_sha256` → `pub fn hmac_sha256` in
`crates/flexcore-audit/src/chain.rs`. The file is BYTE-IDENTICAL to HEAD
(`git diff --stat HEAD -- crates/flexcore-audit/src/chain.rs` is empty)
and both functions are still `pub(crate)` (lines 123 and 193). The edit
was never made, or was lost. Consequence, precise:
`crates/edge-agent/src/update_session.rs` line 54
(`use flexcore_audit::chain::{hex_encode_digest, sha256};`) is a
cross-crate use of a `pub(crate)` item and CANNOT COMPILE — the first
human build fails in `flexcore-edge-agent`. (The `hmac_sha256` half of the
claim is unneeded AND unmade: record 07's `update_evidence.rs` line 42
consumes it crate-internally, which is legal.) NOT FIXED by this agent —
`chain.rs` is outside this subtask's ownership; the orchestrator should
apply the already-recorded one-word edit (or re-scope the verifier into
`flexcore-audit`). The fail-closed default is unaffected: `NoUpdateVerifier`
does not touch `sha256` and is unaffected by this defect.

**F-2 (minor, formatting).** Record 04 §1, last table row, contains an
unescaped pipe inside a cell: `` | `Downloading | VerifiedPendingInstall
→ UpdateFailed` | explicit cancel | … `` — the state name's pipe breaks
the row into two cells and the table's pipe-count consistency (conventions
§8). Not caught by any permitted tool (see F-3). One-character fix for the
orchestrator; not edited here (another agent's record).

**F-3 (method note, not a violation).** `tools/verify_docs.py` checks the
REPORT document only; `tools/validate_agent.py` checks skills/roles;
`check_dependency_rule.py` checks crate edges. Phase-18 records and Rust
files are NOT covered by any permitted tool, so a record's "tools green"
statement validates less than it may appear to. Wave-1 records state the
tool runs honestly, but the coverage boundary is worth a line in the close
record.

**Verified claims (no violation found), with the checks run:**

| Claim (record) | Check run | Result |
|---|---|---|
| Kernel policy byte-identical (06, 08, 21, 25, 27) | `git diff --stat HEAD -- crates/flexcore-kernel/src/revocation.rs` | EMPTY — byte-identical (HEAD = the phases 0–10 commit, so no phase 11–18 edit at all) |
| Kernel policy quoted verbatim (06) | compared against `crates/flexcore-kernel/src/revocation.rs` lines 147–151 (`ROLLOUT_STATEMENT`) and 1211–1213 (`StagedRollout` doc) | VERBATIM |
| Never-granted set byte-identical (08, 27) | `git diff --stat HEAD -- crates/flexcore-kernel/src/manifest.rs` empty; content read | `NEVER_GRANTED_PERMISSIONS` = the frozen four, file byte-identical |
| "Nothing in crates/flexcore-kernel/ was edited" (06, 08, 27) | `git diff HEAD` on the kernel crate | The ONLY kernel changes vs HEAD are `Cargo.toml` + `src/lib.rs`, both P14.16-attributed (phase 14, own decision record) — no phase-18 edit; claim holds as scoped |
| Zero TODO/FIXME/todo!/unimplemented! in every written file (08) | grep over all ten wave-1 files | 0 matches in every file |
| One additive dependency `flexcore-audit` (05, 08) | read `crates/edge-agent/Cargo.toml` line 96 | present |
| `crc32` visibility edit (05, 08) | read `update_verification.rs` line 1092 | present, with the recorded doc note |
| P18.15 seam kept named `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` (04, 08) | read `update_session.rs` `UpdateSessionError::name` | honored (and reused by the interlock's refusal) |
| Consumed P18.12 surfaces exist (06, 25, 27) | grep `fleet_registry.rs` | `FleetStateView::current_cohort` / `current_firmware_state` / `CohortMembership` all present; migration 0023 up/down present with the journal table |
| `KERNEL_REVOCATION_INTERPLAY_STATEMENT` pinned (21) | grep `identity_lifecycle.rs` | present (line 273), test-pinned |
| eSIM vocabulary matches code (24) | grep `esim_lifecycle.rs` | `EsimeProfileState` matches the code's (unusual but consistent) spelling |
| Fail-closed-preserved claims (05, 08) | read `ExtendedUpdateGate` composition + `NoUpdateVerifier` | unchanged semantics; `Sha256UpdateVerifier` is the injected option only — BUT see F-1 for the compile defect behind it |
| Ledger discipline (27: "no checkbox ticked by this agent") | read the ledger | P18.01–P18.13 ticked each WITH a decision-record citation; P18.14–P18.16 unticked — consistent with the orchestrator's post-wave tick, and with both agents' claims |

## 6. Composition-root wiring items (for the coordinator; nothing wired here)

1. Bind `DeviceCommandStateSource` to the P6.16 engine's recorded states:
   the engine's tracking map is private, so the composition root keeps the
   ids it issued and reads `CommandForwardingEngine::lifecycle_state_of(id)`
   per id, wrapping the results in `DeviceCommandObservation`s. (An
   alternative is one additive bulk accessor on the engine — the
   coordinator's call; this module deliberately did not edit
   `command_forward.rs`.)
2. Recorded limitation, honestly carried (the engine's own): the engine's
   tracking state is in-memory and a RESTART CLEARS it — after a restart
   the source reports the (possibly empty) RECORDED set. A deployment that
   cannot reconstruct in-flight command state after restart must keep
   updates blocked until it can; persisting command tracking is P6.18-scope
   work and is NOT solved here.
3. The two-step composition (`authorize_update_attempt` →
   `begin_download(auth.in_flight_count, now)`) is the required order;
   calling `begin_download` with a caller-invented count bypasses the
   interlock and is a composition defect the audit trail would expose.
4. The post-update re-check wiring: after the session reaches `Active`,
   run the P6.13 harness against the composed adapters and record the
   report (`record_conformance_report`); an `AdmitOutcome`-clean report is
   the only unblock.

## 7. UNVERIFIED register (human-gate items)

| # | Item | Status |
|---|---|---|
| U-1 | Everything in this module and both test files is WRITTEN-UNRUN | The first human `cargo build`/`cargo test` is the gate; F-1 will surface there |
| U-2 | The P6.16-engine wiring (§6 item 1) is designed, not composed | No composition root wires the port yet |
| U-3 | The report §5.6 drain discipline is applied to a DEVICE context that §5.6 (plugins) does not literally name | Recorded as the cited discipline carried over, not a §5.6 claim about devices |

## 8. Honest status

The interlock is written-unrun code with complete written-unrun tests; no
command exists, no update was applied, no fleet exists, no hardware
behavior is claimed. The frozen 14-state/27-edge set, the P9.34
fail-closed semantics, and the never-granted set are untouched. The
permitted-tool outputs are quoted verbatim in the close-out record 29.
