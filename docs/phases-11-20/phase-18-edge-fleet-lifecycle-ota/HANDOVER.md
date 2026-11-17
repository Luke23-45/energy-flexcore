# HANDOVER — Phase 18: Edge Fleet Lifecycle and OTA Updates

Status: **implementation complete (expected 16/16), everything WRITTEN-UNRUN.**
The ledger's P18.01–P18.13 ticks are applied (by the orchestrator, citing
records 01–07 and 21–26); the P18.14–P18.16 ticks are the ORCHESTRATOR's
action, applied after this close (program conventions §2 — no agent ever
ticks a checkbox), expected to cite records 41 (wave 2, Agent A), 28 and 29
(wave 2, Agent B), and the close set 42 + 43. The phase gate is **NOT MET**
as a runtime claim: no update bundle has ever been produced, signed, staged,
or applied; no device exists; no fleet exists; no repository is stood up; no
key exists; no migration has run; no test has run — the first human build of
the Phase 0–10 workspace is the verification gate for every statement below.

## What was implemented (all WRITTEN, never run)

| Area | Subtasks | Records | Delivered (written) |
|---|---|---|---|
| OTA architecture + format + repository plan (wave 1, Agent A) | P18.01–P18.03 | 01, 02, 03 | ADR-1801 (TUF-style metadata repository + SUIT-shaped signed manifests [S-161][S-162]; **P18-E-1 registered OPEN, DEFAULT (b)**); the `flexcore-edge-update-manifest-v2` format (`crates/edge-agent/src/update_bundle.rs`: applicability, sequence anti-rollback, per-payload digests, size caps, the 14-variant error vocabulary, the durable `AntiRollbackCounter`); the repository role/key plan (root offline, delegations, snapshot/timestamp, N+1 root rotation — every key ceremony a human gate, P10.21) |
| Device flow + edge extension + orchestrator + evidence (wave 1, Agent A) | P18.04–P18.07 | 04, 05, 06, 07 | the recorded update flow (`update_session.rs`: closed state machine, 6 named failure states, the P18.15 blocking rule consumed by citation); the extended gate (`ExtendedUpdateGate` — the 9-step refusal order with the P9.34 gate preserved VERBATIM as the inner step; `Sha256UpdateVerifier` consuming the P11.15 std-only SHA-256, fail-closed `NoUpdateVerifier` default untouched); the device-fleet cohort orchestrator (`crates/flexcore-application/src/rollout_orchestrator.rs`: pre-registered canary band + abort thresholds, per-device attestation before authorization, **P18-E-3's DEFAULT implemented as configuration-shaped mechanism**; record 06 = the scope-distinction record vs the byte-identical kernel policy); the per-attempt evidence pack (`crates/flexcore-audit/src/update_evidence.rs`: five closed kinds, HMAC-SHA-256 per-attempt chain domain-separated from the P11.15 chain, §10.1 mapping) |
| Identity / zero-touch / decommissioning / eSIM / registry / attestation (wave 1, Agent B) | P18.08–P18.13 | 21, 22, 23, 24, 25, 26 | IDevID/LDevID mapped onto the P9.33 ceremony (`identity_lifecycle.rs`); FDO-pattern zero-touch with the fail-closed `NoRendezvousConfigured` shipped default (`zero_touch.rs` — the manual P9.33 path stays default AND fallback); the decommissioning runbook (record 23, record-only); the eSIM data model (`esim_lifecycle.rs`, [S-167]); migration 0023 + the fleet-state schema types (`fleet_registry.rs` — closed vocabularies, latest-row-wins projection); the attestation policy (`attestation.rs` — ordered named refusals, the honest hardware-absent fallback, **P18-E-2 registered OPEN, DEFAULT (a)**, PENDING PILOT HARDWARE SPEC per P6.34) |
| Update test suite (wave 2, Agent A) | P18.14 | 41 | the mock-device TEST DOUBLE (`tests/p18_14_support/mod.rs`) + four scenario batteries, 65 integration tests: manifest validation + the anti-rollback negatives; rollback scenarios + the three power-loss (interrupted-update) simulations; the attestation ordered-refusal + fleet-vocabulary battery; the concurrent-command blocking battery consuming the LANDED P18.15 rule (the two-step composition of record 28 §4). Plus the recorded F-1/F-2 defect resolutions (record 41 §2) |
| The update/command-lifecycle interlock (wave 2, Agent B) | P18.15 | 28, 29 | `crates/edge-agent/src/update_command_interlock.rs`: the blocking rule made REAL (non-terminal §14.1 states block; a strict superset of the ISSUED/TRACKING citation, the strengthening recorded in 28 §2; `DeviceStateUnknown` fails closed), the report §5.6 drain procedure (OBSERVER-only), the post-update conformance re-check consuming the P6.13 harness (a clean report proves the §19.2(6) contract ONLY — the P6.19 boundary); B's integration file `tests/update_command_interlock_p18_15.rs`; B's honesty review of the wave-1 records (28 §5: F-1 build-blocking, F-2, F-3) |
| Decision records + close (wave 2, Agent A) | P18.16 | 42, 43 | the verified E-decision register (P18-E-1/E-2/E-3 all OPEN with DEFAULTs in both register locations — record 42, nothing closed by any agent); the close + consistency audit (record 43: pipe counts, [S-n] resolution, honesty sweep, byte-identical invariants, F-1/F-2 resolved, F-4 recorded for the orchestrator) |

## Blockers carried (human gate inputs)

1. **The first human build** (`cargo build --workspace` → clippy →
   `cargo test --workspace`): every Rust artifact in this phase is written
   against the WRITTEN-UNRUN 0–10 base. F-1 (the missing `sha256`
   visibility edit) was the build blocker and is RESOLVED (record 41 §2);
   the first build may surface more recorded defects — fixed as recorded
   edits, never silently.
2. **No device exists and none can exist within this program**: no fleet,
   no enrollment, no claim, no attestation, no rollout, no decommissioning;
   every flow is validated against test doubles only (plan §4 non-goals).
3. **No update bundle is produced, signed, staged, or applied** — the
   P9.34/P10.12 gap is closed at DESIGN level only (plan §8 criterion 2,
   quoted verbatim in the close record §headline).
4. **No production signing key exists**; the repository is not stood up;
   every key ceremony is a human gate (P10.21 interplay; record 03 §4).
5. **The three E-decisions are OPEN** (P18-E-1, P18-E-2, P18-E-3 — record
   42 compiles the register; every closer is the human owner; every closer
   input is EMPTY / NOT-STARTED).
6. **The P6.34 hardware posture stands**: every hardware determination is
   PENDING PILOT HARDWARE SPEC; the attestation assumption set stays open
   until the pilot hardware spec fills it (P18-E-2).
7. **The UNVERIFIED registers** (records 01/02/03/04/21/22/24: the TUF
   role/rotation semantics, the strictly-ahead sequence comparison, the
   SUIT concept alignments, the OCA-text enumeration, the 802.1AR/FDO and
   SGP.22/SGP.32 specifics) are human verification items — never closed
   from the wave summaries.
8. **Orchestrator items from the audit**: F-4 — the unescaped literal pipe
   in `records/24_p18_11_esim_lifecycle.md`'s observation-row cell
   (record 43 §4.2; one-character-class fix, outside every agent's edit
   scope).

## Expected orchestrator citations (ledger ticks, applied AFTER this close)

| Ledger row | Expected citation (decision record) |
|---|---|
| P18.14 | `records/41_p18_14_update_test_suite_mock_device.md` |
| P18.15 | `records/28_p18_15_update_command_interlock.md` (the decision record; `records/29_p18_15_wave2_agent_b_close_out.md` named alongside) |
| P18.16 | `records/42_p18_16_decision_records_and_close.md` (the decision-register half) and `records/43_phase_18_close_and_consistency_audit.md` (the close/audit half); the recommended tick citation for the row is the close record 43, with 42 named alongside |

(The wave-2 record numbering landed as 41 + close-set 42–43 for Agent A and
28–29 for Agent B — the same one-record-per-subtask shape wave 1 used.
Records 30–40 and 44–59 remain free.)
