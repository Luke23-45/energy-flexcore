# Record 08 — Phase 18, Wave 1, Agent A close-out (P18.01–P18.07): OTA pipeline

| Field | Value |
|---|---|
| Wave / agent | Phase 18 wave 1, Agent A |
| Subtasks | P18.01 (record 01, P18-E-1 registered OPEN with DEFAULT (b)), P18.02 (record 02), P18.03 (record 03), P18.04 (record 04), P18.05 (record 05), P18.06 (record 06 — the scope-distinction record), P18.07 (record 07) |
| Status | WRITTEN-UNRUN / IMPLEMENTED-UNRUN: all deliverables are decision records + design-baseline additive code with in-module written-unrun test modules; nothing compiled, nothing executed, **no update bundle produced/signed/staged/applied, no device enrolled/updated/attested, no cohort rolled out, no fleet exists** |
| Bans honoured | No installs; no builds; no tests run (execution = human gate); zero `todo!()`/`unimplemented!()`/TODO/FIXME in every file written (grep-verified, exit 1 = no matches); permitted verification tools only, outputs quoted verbatim below; **no edits** to `ledger_phase_18.md`, `plan.md`, `context.md`, `docs/foundations/*`, `docs/plan/*`, `.agent/*` — every ledger checkbox remains unticked by this agent |
| Byte-identity | `crates/flexcore-kernel/src/revocation.rs` byte-identical (git diff vs HEAD: empty — verified); the fail-closed P9.34 default semantics preserved (`NoUpdateVerifier` unchanged); the never-granted set untouched |

## 1. Files created / modified

**NEW (edge-agent, P18.02/P18.04/P18.05):**

| File | Content |
|---|---|
| `crates/edge-agent/src/update_bundle.rs` | The `flexcore-edge-update-manifest-v2` format extending the P9.34 `UpdateManifest` (composed, not forked): `UpdateApplicability` (vendor/device_class mandatory match; closed empty-vs-listed allowlist semantics), `PayloadDigest` (offset/size/digest, no overlaps), `ExtendedUpdateManifest` (mandatory expiry, sequence ≥ 1, size caps), `UpdateBundleError` (14 named variants), `AntiRollbackCounter` (append-only CRC-32-framed monotonic register, truncate-and-audit, durable-before-memory), `UPDATE_BUNDLE_STATEMENT` + `ANTI_ROLLBACK_SEMANTICS_STATEMENT` (UNVERIFIED labels pinned), in-module written-unrun tests |
| `crates/edge-agent/src/update_session.rs` | P18.04: `UpdateProgressState` (closed, terminal states terminal), `UpdateSessionStateMachine` (recorded flow with the P18.15 blocking rule consumed by citation — `CommandsInFlight` refusal), closed failure-state vocabulary (6 named constants), `UpdateStatusNotification`. P18.05: `Sha256UpdateVerifier` (CONSUMES the P11.15 std-only SHA-256 from `flexcore_audit::chain` — never duplicated; FIPS 180-4 vectors pinned in tests), `ExtendedUpdateGate` (the 9-step recorded refusal order with the P9.34 gate preserved VERBATIM as the inner step; one verifier instance feeds both layers; durable counter advance before any verdict), `UPDATE_SESSION_STATEMENT` + `UPDATE_FLOW_HARDWARE_STATEMENT`, in-module written-unrun tests incl. the fail-closed-preserved test |

**NEW (flexcore-application, P18.06):**

| File | Content |
|---|---|
| `crates/flexcore-application/src/rollout_orchestrator.rs` | `RolloutPlan::plan` (pre-registration STRUCTURAL: thresholds + canary band 500..=1000 bp required at plan time — X-8; the 5–10% band is a DEFAULT RECOMMENDATION, not a sourced figure), `AbortThresholds`, `CohortPlan` (canary first, no skipping), `FleetCohortDirectory` port (P18.12 consumed by citation — the wait-check PASSED, binding described to `fleet_registry::FleetStateView::current_cohort`/`CohortMembership`), `AttestationVerdict` (P18.13 `AttestationDecision` mapped by citation), `DeviceUpdateOutcome`, `RolloutOrchestrator` (authorization requires membership + PASSED attestation + active cohort; outcome recording with per-evaluation audited threshold; terminal abort; declared-order advancement to `RolloutComplete`), `RolloutAuditSink` port + `MemoryRolloutAuditSink`, `FleetRolloutSnapshot`/`CohortSnapshot` (visualization data), `DEVICE_FLEET_ROLLOUT_SCOPE_STATEMENT` + `ABORT_SEMANTICS_STATEMENT`, in-module written-unrun tests |

**NEW (flexcore-audit, P18.07):**

| File | Content |
|---|---|
| `crates/flexcore-audit/src/update_evidence.rs` | `UpdateEvidenceRecord` (mandatory detail, P0.09 ordering mirrored), the closed five-kind vocabulary (attempt/verify/install/result/rollback — `from_code` refuses everything else), `UpdateEvidenceOutcome`, `UpdateEvidenceError` (9 named variants), per-attempt HMAC-SHA-256 chain (domain-separated from the P11.15 audit chain, SAME std-only primitives, sequence inside the canonical bytes), `UpdateEvidenceChain` (append-only, attempt-mixing refused, empty-export refused), `UpdateEvidenceExport` + honesty wording, `to_audit_record` §10.1 mapping (rides the EXISTING `AuditSink` + P11.15 chain — consumed, never redefined), `UPDATE_EVIDENCE_STATEMENT`, in-module written-unrun tests |

**MINIMAL EDITS (additive/visibility-only):**

| File | Change |
|---|---|
| `crates/edge-agent/src/update_verification.rs` | `fn crc32` → `pub(crate) fn crc32` (visibility only; the P18.02 counter framing reuses it; doc note added; behavior unchanged) |
| `crates/flexcore-audit/src/chain.rs` | `pub(crate) fn sha256` / `pub(crate) fn hmac_sha256` → `pub` (visibility only; FIPS/RFC implementations and vector tests untouched) |
| `crates/edge-agent/src/lib.rs` | Two additive module entries (`update_bundle`, `update_session`) with honesty headers (re-read before edit; Agent B's concurrent entries preserved) |
| `crates/edge-agent/Cargo.toml` | One additive dependency `flexcore-audit = { workspace = true }` (composition→core, checker-legal; consumed-not-duplicated rationale documented) |
| `crates/flexcore-audit/src/lib.rs` | One additive module entry (`update_evidence`) |
| `crates/flexcore-application/src/lib.rs` | One additive module entry (`rollout_orchestrator`) |

**Records written:** `01_p18_01_ota_architecture_adr.md` (ADR-1801; P18-E-1 OPEN, DEFAULT (b)), `02_p18_02_update_bundle_format.md`, `03_p18_03_metadata_repository_plan.md`, `04_p18_04_device_update_flow.md`, `05_p18_05_edge_agent_update_extension.md`, `06_p18_06_rollout_orchestrator_scope_distinction.md` (kernel policy quoted verbatim, separately argued), `07_p18_07_update_evidence_audit_pack.md`, this close-out (08). **No new crates; no checker/root-manifest edits; no workspace-member changes.**

## 2. Verbatim permitted-tool outputs (final state, all exit 0)

```text
$ python -I tools/check_dependency_rule.py
OK ù dependency rule holds for 29 workspace members.
EXIT=0

$ python tools/verify_docs.py
report: docs\plan\Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC
EXIT=0

$ python tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid
EXIT=0
```

(The `ù` in the checker line is the console rendering of the source's
em-dash; the reported member count is unchanged at 29 — this agent added
no crate and no manifest member.)

## 3. Deviations and honesty notes

1. **No deviations from the brief's bans or scope.** One judgment call
   documented in record 05 §2: consuming the P11.15 SHA-256 required two
   VISIBILITY-ONLY edits (`sha256`/`hmac_sha256` in flexcore-audit,
   `crc32` in update_verification) and one additive composition→core
   dependency edge — the alternative (reimplementing SHA-256 in
   edge-agent) would have violated the consume-don't-duplicate ban,
   which wins.
2. **WRITTEN-UNRUN defects are possible** (the whole workspace has never
   compiled): the four new modules were hand-reviewed twice after
   writing (a corrupt-tail counter plumbing fix in the counter, an
   unused-import/mut cleanup, a trait-object `lines()` type error in a
   test, a `String`-to-error conversion fix, an `Arc` sink adapter, and
   a `RolloutComplete` named variant were found and fixed) — the first
   human build remains the gate and may surface more.
3. **Wait-check for B's P18.12 schema PASSED**: Agent B's
   `fleet_registry`/`attestation` modules and records 21–26 landed
   during this wave; the orchestrator's consumed-port documentation cites
   the concrete surfaces (`FleetStateView::current_cohort`,
   `CohortMembership`, `AttestationDecision`) and explains why the ports
   exist (the dependency rule forbids core→composition edges).
4. **Shared-file concurrency:** `crates/edge-agent/src/lib.rs` was
   modified by Agent B mid-edit once; it was re-read and the additive
   entries re-applied — both agents' entries are present and
   independent.
5. **UNVERIFIED everywhere it belongs:** TUF role semantics, the N+1
   rotation procedure, the strictly-ahead sequence comparison semantics,
   the SUIT concept alignments, and the OCPP 2.0.1 firmware-status
   mapping all carry UNVERIFIED labels with named implementation-time
   checks (records 01 §5, 02 §4, 03 §6, 04 §4); no OCA text was consulted
   this wave and no OCPP status name is claimed.
6. **Acceptance-criteria echoes (plan §8):** no bundle
   produced/signed/staged/applied (criterion 2); no device/fleet
   (criterion 3); fail-closed preserved by construction with the
   default quoted in record 05 §1 (criterion 4); kernel policy
   byte-identical with the device-fleet mechanism separately argued in
   record 06 (criterion 5); no production signing key and every key
   ceremony a human gate (criterion 7); the three permitted tools green
   (criterion 8's tool set, quoted above).

## 4. Handoffs

**To Agent B (wave 1, identity/registry/attestation):**
- The orchestrator consumes your surfaces through injected ports — the
  composition-root binding should map `FleetStateView::current_cohort` →
  `FleetCohortDirectory::cohort_members` (the empty-cohort and
  duplicate-id refusals in `load_members` expect journal-backed views)
  and `AttestationDecision::AuthorizeUpdate` →
  `AttestationVerdict::Passed { evidence_reference }`.
- `update_session.rs`'s blocking-rule parameter
  (`begin_download(issued_or_tracking_commands, …)`) is the seam your
  P18.15 interlock (wave 2 B) formalizes — keep the refusal named
  `UPDATE_BLOCKED_COMMANDS_IN_FLIGHT`.

**To wave 2 (Agent A — P18.14/P18.16; Agent B — P18.15):**
- P18.14's test suite should import the in-module negative batteries
  (anti-rollback replay refusal, payload-digest mismatch, fail-closed
  default) as its scenario subset and add the interrupted-update
  (power-loss) simulation against `FileUpdateLedger` +
  `AntiRollbackCounter` reopen-replay semantics.
- P18.16 compiles record 06's scope-distinction argument and the P18-E-1
  DEFAULT into the close record; both P18-E-1 and P18-E-3 remain OPEN
  (human closers); P18-E-2 is Agent B's register.
- The UNVERIFIED registers (records 01/02/03/04) are wave-2/human
  verification items — the TUF/SUIT spec-text checks and the OCA-text
  enumeration must not be closed from this wave's summaries.

**To the human gates:** first workspace build; test execution; the
P18-E-1 fleet-size threshold recording; all key ceremonies (P10.21);
all UNVERIFIED register items.
