# Record 41 — P18.14: The update test suite (written-unrun) — the mock device and the four scenario batteries

Date: 2026-11-17. Subtask: P18.14 (`ledger_phase_18.md` §P18.14; phase-18
plan §2 row P18.14). Status: **IMPLEMENTED, AWAITING HUMAN BUILD/TEST**
(WRITTEN-UNRUN / TEST-DOUBLE depth per plan §1; conventions §2.2).
**No device exists — the mock is a TEST DOUBLE in test code. No update has
ever been produced, signed, staged, or applied; no fleet exists; every test
below is UNRUN and its execution is a human gate (loop ban 3).**

| Field | Value |
|---|---|
| Files touched | `crates/edge-agent/tests/p18_14_support/mod.rs` (NEW — the mock-device test double + shared fixtures); `crates/edge-agent/tests/p18_14_manifest_and_antirollback.rs` (NEW, 28 tests); `crates/edge-agent/tests/p18_14_rollback_and_power_loss.rs` (NEW, 10 tests); `crates/edge-agent/tests/p18_14_attestation_and_fleet_vocab.rs` (NEW, 19 tests); `crates/edge-agent/tests/p18_14_concurrent_command_blocking.rs` (NEW, 8 tests) — 65 tests total, all WRITTEN-UNRUN |
| Consumed by citation, never redefined | the P9.34 surfaces (`update_verification.rs`: gate, ledger, activation, `NoUpdateVerifier`, `UPDATE_SLOT_DECISION`); the P18.02 format (`update_bundle.rs`: v2 manifest, counter, the 14-variant error vocabulary); the P18.04/P18.05 flow (`update_session.rs`: state machine, failure-state vocabulary, extended gate, `Sha256UpdateVerifier`); the P18.12 schema (`fleet_registry.rs`); the P18.13 policy (`attestation.rs`); **Agent B's LANDED P18.15 interlock** (`update_command_interlock.rs`, record 28 §4 surface); the P11.15 std-only SHA-256 (`flexcore_audit::chain`) |
| Wait-check | PASSED: B's P18.15 landed during this agent's session (module + records 28/29 + B's own integration file `tests/update_command_interlock_p18_15.rs`); the concurrent-command tests consume the record-28-§4 surface and cite ONE stable code (`UPDATE_BLOCKED_COMMANDS_IN_FLIGHT`) with the wave-1 session rule |

## 1. What was delivered (test-double depth)

| Suite | Tests | Covers |
|---|---|---|
| `p18_14_support/mod.rs` | fixture module (not a test target) | `MockUpdateDevice`: one in-process TEST DOUBLE composing the real update surfaces over temp-dir files — the P18.05 extended gate, the P18.02 counter, the P9.34 register, the activation surface, a per-attempt session; `MockVerifierChoice::{NoneConfigured, TestOnlyFnv1a64, Sha256}` (the fail-closed default, the loudly-labelled test-grade verifier, and the SHA-256 option — the DEPLOYMENT REQUIREMENT wiring exercised as code only); `power_cycle()` recomposes from DISK state only; the attestation/fleet fixtures; a scratch-dir/cleanup harness (nothing written outside `std::env::temp_dir()`) |
| `p18_14_manifest_and_antirollback.rs` | 28 | the 14-variant `UpdateBundleError` vocabulary pinned; structural validation (zero sequence, duplicate names, overlapping ranges, zero-size payloads, entry/total caps, base-manifest violations riding the v2 reason, mandatory expiry); applicability (exact vendor/class match, the closed empty-vs-listed allowlist, NO wildcard, refusal BEFORE digest work); expiry at/after the injected instant and after a simulated outage; ANTI-ROLLBACK NEGATIVES: replay refusal, equal/lower sequence, tampered replay, reopen-replay monotonicity, torn-tail truncate-and-count, foreign file refused, refused-verdict-advances-nothing, the fail-closed default (`UPDATE_VERIFIER_REFUSED` riding through, both audit layers, zero counter movement); the SHA-256 round trip with the FIPS 180-4 "abc" vector; unsigned-first; P9.34 authority preserved |
| `p18_14_rollback_and_power_loss.rs` | 10 | the full recorded flow interleaved (session → gate → register → Active); duplicate-activation refusal; the AUTO-ROLLBACK flow (health-failed → RollingBack → register rollback with the recorded reason → RolledBack, active version reverting); impossible rollback targets by name (never-activated, not-behind); the below-floor refusal without an override and the override riding the record AND the audit line; POWER-LOSS simulations: (1) between the durable counter advance and the install — the manifest becomes a replay on reboot, recovery requires re-issued metadata with a higher sequence (the recorded fail-safe direction, `ANTI_ROLLBACK_SEMANTICS_STATEMENT`); (2) mid-append on the register — torn tail truncated and counted, the good activation survives; (3) after activation before health — rollback targets intact by replay; a cancelled session leaves ZERO register records; foreign-file and empty-version register refusals |
| `p18_14_attestation_and_fleet_vocab.rs` | 19 | the P18.13 gate's ORDERED refusals (absent → revoked → device mismatch → stale → counter regression → measurement check → anchor requirement), each named; baseline equality refusing missing/altered/EXTRA components; the reference-register mode refusing a scheme change; `NoMeasurementCheck` recording that nothing is asserted; the P18-E-2 DEFAULT not requiring an anchor and a requirement refusing software-only evidence BY NAME; invalid policy/evidence errors; the stable decision names; the P18.12 vocabularies (five kinds, firmware/health/attestation closed sets with exact-case refusals, the `IN:`/`OUT` cohort encoding with its parse refusals, row-shape refusals, latest-observation-wins projection, invalid rows never entering); the journal↔gate cross-link (REVOKED/FAILED journal states never satisfy the gate) |
| `p18_14_concurrent_command_blocking.rs` | 8 | the CONCURRENT-COMMAND battery consuming B's landed rule: ISSUED/TRACKING blocks BEFORE the extended gate is reached (no verification audited, counter unmoved, register empty); the drain-gap states (ACKNOWLEDGED/FAILED/EXPIRED) block per the recorded strict-superset strengthening; an unknown device command state fails closed; the drain walk (ISSUED → the operator's frozen-lifecycle action simulated in the SOURCE, never written by the update path → drained → the two-step composition `begin_download(0, now)` → the extended gate verifies → a second attempt re-reads the state); the post-update conformance re-check gating the NEXT update (`UPDATE_CONFORMANCE_RECHECK_OUTSTANDING` with zero commands in flight; a CLEAN P6.13 report re-opens authorization; the audit line carrying the never-protocol-conformance boundary); the wave-1 session refusal and the explicit verification-refused failure state composing correctly; the observer-only pin |

Every test hand-computes its expected outcome against the recorded rules
(the P9.34 integration-suite discipline); every file carries the honesty
header restating the no-device / no-update / no-fleet boundary.

## 2. Recorded findings resolved while closing (B's record 28 §5, Agent A's files)

| Finding | Resolution |
|---|---|
| **F-1 (build-blocking)** — `crates/flexcore-audit/src/chain.rs` `sha256` still `pub(crate)`; record 05 §2 / record 08 §1 claim the visibility edit but it was never applied; `update_session.rs`'s cross-crate `use flexcore_audit::chain::sha256` cannot compile | **RESOLVED** at this close: `sha256` → `pub` (the one-word edit record 05 documents), with a visibility note citing record 05 §2 and F-1; behavior unchanged. The `hmac_sha256` half of the claim is deliberately NOT applied — record 28 §5 is right that `update_evidence.rs` consumes it crate-internally (legal); record 08 §1's claim of both edits is corrected by this close record (the minimal correct change is `sha256` only) |
| **F-2 (minor, formatting)** — record 04 §1's last table row carries an unescaped pipe inside the cancel-row state cell | **RESOLVED**: the cell now reads `` `Downloading / VerifiedPendingInstall → UpdateFailed` `` — same meaning, the pipe-count consistency restored (confirmed by the close audit's scan) |

## 3. Honest status and bounds

- Every artifact is WRITTEN-UNRUN; nothing compiled, nothing executed; the
  first human build/test pass is the gate and may surface recorded defects
  (the whole workspace has never compiled — F-1 is the one already found).
- The mock device is a TEST DOUBLE: it composes the real library functions
  over temp-dir files; it cannot touch a network, hold key material,
  produce or sign a bundle, or represent hardware. The A/B-slot/watchdog/
  boot-counter behavior in the scenarios is a fixture value, not hardware —
  every hardware determination stays PENDING PILOT HARDWARE SPEC (P6.34;
  `UPDATE_FLOW_HARDWARE_STATEMENT`).
- The digests in the fixtures are the loudly-labelled test-grade FNV-1a-64
  (`fnv1a64-test`, NOT a security primitive) or the consumed P11.15
  SHA-256; **no production signing key exists** and none is needed by any
  test; every key ceremony stays a human gate (P10.21).
- The strictly-ahead sequence semantics remain UNVERIFIED against the
  TUF/SUIT spec texts [S-161][S-162] — the tests pin that the code and
  `ANTI_ROLLBACK_SEMANTICS_STATEMENT` agree with EACH OTHER, never that
  they match the standards; the OCPP status-name mapping stays unclaimed.
- No `todo!()`/`unimplemented!()`/TODO/FIXME in any file (grep-verified,
  exit 1 = no matches).

## 4. Handoff

- The suites are the P18.16 close record's negative-test inventory; the
  orchestrator's P18.14 tick is expected to cite this record (see
  `HANDOVER.md`'s expected-citation table).
- To the human gates: run `cargo test -p flexcore-edge-agent` at the first
  build; any failure is a recorded edit, never a silent reinterpretation.
