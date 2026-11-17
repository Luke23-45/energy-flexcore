# P18.15 (wave 2, Agent B) — Close-out record: the update/command-lifecycle interlock + the wave-1 honesty review

| Field | Value |
|---|---|
| Wave / agent | Phase 18 wave 2, Agent B |
| Subtask | P18.15 (decision record: `28_p18_15_update_command_interlock.md`) plus the assigned review of the wave-1 honesty statements (records 01–08, 21–27 — findings in record 28 §5) |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (WRITTEN-UNRUN; conventions §2.2). **No command exists; no update has ever been applied; no fleet exists; no command and no update interact on hardware.** |
| Bans honoured | No installs; no builds; no tests run (execution = human gate); zero `todo!()`/`unimplemented!()`/TODO/FIXME in every file written (grep-verified, 0 matches); permitted verification tools only, outputs quoted verbatim below; no edits to `ledger_phase_18.md`, `plan.md`, `context.md`, `docs/foundations/*`, `docs/plan/*`, `.agent/*`, any other agent's records, or any wave-1 code file. No checkbox ticked anywhere. |
| Frozen sets | `crates/flexcore-kernel/src/revocation.rs` byte-identical (git diff vs HEAD empty — verified); the frozen 14-state/27-edge command set CITED (`command_path::FROZEN_COMMAND_STATE_NAMES` + the frozen terminal five), never forked; the P9.34 fail-closed semantics untouched (no edit to `update_verification.rs`); the never-granted set untouched (`manifest.rs` byte-identical vs HEAD; content read and confirmed = the frozen four) |

## 1. Files created / modified

| File | Change |
|---|---|
| `crates/edge-agent/src/update_command_interlock.rs` | NEW — the P18.15 interlock: the blocking rule (non-terminal frozen §14.1 states, strict superset of the recorded ISSUED/TRACKING citation, fail-closed on unknown device state), the report §5.6 drain procedure (observer-only), the post-update conformance re-check (P6.13 consumed), `INTERLOCK_STATEMENT`, complete in-module written-unrun tests |
| `crates/edge-agent/tests/update_command_interlock_p18_15.rs` | NEW — the integration test surface for P18.14: blocking battery, the two-step composition with the UNCHANGED `UpdateSessionStateMachine::begin_download`, the full drain walk (ISSUED → operator-cancel → drained → authorized), observer-only pin, the re-check battery incl. the REAL P6.13 harness run, statement pins — all WRITTEN-UNRUN |
| `crates/edge-agent/src/lib.rs` | ONE additive module entry (`update_command_interlock`) with an honesty header; re-read immediately before the edit |
| `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/28_p18_15_update_command_interlock.md` | NEW — the decision record (exact surface for P18.14 itemized in §4; honesty-review findings in §5) |
| `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/29_p18_15_wave2_agent_b_close_out.md` | NEW — this record |

Untouched by design: `update_session.rs`, `update_verification.rs`,
`update_bundle.rs`, `command_path.rs`, `command_forward.rs`, `runner.rs`,
`conformance.rs`, `ports.rs` (the closed `EdgeAuditKind` vocabulary was
consumed, not extended), everything under `crates/flexcore-kernel/`,
`crates/flexcore-audit/`, `crates/flexcore-application/`, the migrations,
the ledger, and every other agent's record.

## 2. Verbatim permitted-tool outputs (final state, all exit 0)

```text
$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0

$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
EXIT=0
```

(The `ù` in the checker line is the console rendering of the source's
em-dash, as in records 08 and 27; 29 workspace members — this agent added
no crate and no manifest member.)

## 3. Honesty-review findings (full detail and the verification table in record 28 §5)

1. **F-1 (SIGNIFICANT, build-blocking):** the record-05/08 visibility edit
   in `crates/flexcore-audit/src/chain.rs` (`sha256`/`hmac_sha256` →
   `pub`) is NOT in the file (byte-identical vs HEAD; both still
   `pub(crate)` at lines 123/193). As written,
   `crates/edge-agent/src/update_session.rs:54` cannot compile (cross-crate
   private-item use). NOT FIXED (outside this subtask's ownership) — the
   orchestrator should apply the already-recorded one-word edit or re-scope
   the verifier into `flexcore-audit`. The fail-closed default is
   unaffected.
2. **F-2 (minor, formatting):** record 04 §1 last table row has an
   unescaped pipe inside a cell, breaking the table's pipe-count
   consistency (conventions §8).
3. **F-3 (method note):** the permitted tools do not cover phase records
   or Rust files — a record's "tools green" statement validates less than
   it may appear to (verify_docs = the report only).
4. No violation of the not-produced / not-applied / no-fleet class was
   found in any wave-1 record; the kernel-policy, never-granted,
   fail-closed-preserved, UNVERIFIED-register, and scope-distinction
   statements all verified against the code (the checks and results are
   tabulated in record 28 §5).

## 4. Deviations and honesty notes

1. **The blocking rule is a recorded STRENGTHENING of the citation** (not
   a deviation from the plan): the plan row says "ISSUED/TRACKING"; the
   implemented rule blocks every NON-TERMINAL frozen state (also
   ACKNOWLEDGED/FAILED/EXPIRED — the drain gap), which is strictly safer;
   the delta is stated verbatim in record 28 §2 and the refusal keeps the
   recorded stable name `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` (the handoff
   requirement from record 08 §4, honored).
2. **No edit to `command_forward.rs`**: the production state-source wiring
   cannot enumerate the engine's private tracking map, so the port is
   wired at the composition root (record 28 §6 item 1) instead of adding
   an accessor to a file this subtask does not own. Recorded as a
   coordinator item, honestly.
3. **WRITTEN-UNRUN defects are possible** (the workspace has never
   compiled): the module and both test files were hand-reviewed twice
   after writing (a bogus helper-type reference and one audit-detail
   construction were found and fixed during writing; one `unreachable!`
   was restructured away). The first human build remains the gate and may
   surface more — including F-1, which will fail the edge-agent build
   until the orchestrator applies the missing one-word edit.
4. The drain procedure is OBSERVER-ONLY by design (the operator owns
   "finish or cancel pending operations" through the frozen lifecycle);
   this is stated in the module header, the record, and pinned by a test.

## 5. Handoffs

**To Agent A (P18.14 + P18.16):**
- The exact surface to cite is record 28 §4 (types, signatures, refusal
  names, audit-event names, the two-step composition). The
  concurrent-command tests should compose
  `InMemoryCommandStateSource::reporting(...)` +
  `UpdateCommandInterlock::authorize_update_attempt()` and assert the
  `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT` refusal, then the two-step
  `begin_download(auth.in_flight_count, now)` composition.
- The integration file `tests/update_command_interlock_p18_15.rs` is the
  citable written-unrun scenario set for the blocking/drain/re-check
  legs.
- For the P18.16 close: F-1 (the missing `chain.rs` visibility edit) MUST
  be resolved before any claim that the workspace builds; F-2 and F-3 are
  recorded for the consistency audit.

**To the orchestrator:**
- Apply the record-05-documented one-word visibility edit to
  `crates/flexcore-audit/src/chain.rs` (or re-scope) — the precise defect
  is F-1 in record 28 §5.
- The composition-root wiring items (state-source binding, restart-clears
  limitation, two-step order, re-check wiring) are record 28 §6.
- P18.15's ledger line stays unticked (tick + citation is the
  orchestrator's act, as with P18.01–P18.13).

**To the human gates:** first workspace build (blocked by F-1 until the
one-word edit lands); test execution; the P18.16 consistency audit; all
UNVERIFIED register items (record 28 §7).
