# Phase 18 — Edge Fleet Lifecycle and OTA Updates: Exit-Gate Checklist

**Exit gate (plan §8, the phase's own acceptance criteria): format specs,
flow specs, orchestrator/module designs, runbooks, policy records, and test
files exist WRITTEN-UNRUN — no subtask claims a run, a test pass, or
verified behavior; no update bundle is produced, signed, staged, or
applied; no device exists; fail-closed is preserved by construction; the
kernel plugin-rollout policy stays byte-identical with the device-fleet
mechanism separately argued; updates are specified as REFUSED while any
command is ISSUED/TRACKING; no production signing key exists; the permitted
tools stay green; P18-E-1/E-2/E-3 remain OPEN with DEFAULTs recorded.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
produced, signed, staged, applied, or certified** — the first human build
is the program gate; every status below is a written-artifact status, never
a runtime claim.

## Plan §8 acceptance criteria (one-for-one)

- [x] **Criterion 1 — format specs, flow specs, orchestrator and module
  designs, runbooks, policy records, and test files exist written-unrun; no
  subtask claims a run, a test pass, or verified behavior.** Evidence: the
  16/16 coverage table below; every record's status field is
  WRITTEN-UNRUN / RECORD-ONLY / TEST-DOUBLE / REGISTER COMPILATION / CLOSE
  RECORD; the honesty sweep (close record 43 §4.4) found zero run/pass
  claims in any status field. Status: **satisfied by the written records
  and the audit**.
- [x] **Criterion 2 — no update bundle is produced, signed, staged, or
  applied; the P9.34/P10.12 gap is closed at DESIGN level only, and the
  close record says so verbatim.** Evidence: record 43's headline quotes
  the criterion verbatim; the claim-pattern sweep found zero violations;
  the code-level statements (`UPDATE_BUNDLE_STATEMENT`,
  `UPDATE_SESSION_STATEMENT`, `UPDATE_EVIDENCE_STATEMENT`) pin the
  never-produced boundary in source. Status: **satisfied by the audit** as
  of the close record's date (an inspection, not a tool run).
- [x] **Criterion 3 — no device exists: nothing is enrolled, claimed,
  updated, rolled out, attested, or decommissioned; all validation is test
  doubles.** Evidence: the mock device is a labeled TEST DOUBLE (record 41;
  `tests/p18_14_support/mod.rs` header); B's `InMemoryCommandStateSource`
  is a labeled TEST DOUBLE (record 28); the identity-path records carry
  the no-fleet statement (close record 43 §4.4). Status: **satisfied by
  the audit**.
- [x] **Criterion 4 — fail-closed preserved by construction: with no
  configured verifier the agent refuses updates (`NoUpdateVerifier`
  semantics unchanged); the close record quotes the refusal behavior as
  specified.** Evidence: record 43 §3 quotes the `NoUpdateVerifier`
  implementation verbatim; the composed refusal
  (`UPDATE_VERIFIER_REFUSED` riding through as `BaseVerificationRefused`,
  zero counter movement) is pinned by a P18.14 test; the DEFAULT
  composition is `NoUpdateVerifier` everywhere. Status: **satisfied by the
  quoted behavior and the pinned test**.
- [x] **Criterion 5 — the kernel plugin-rollout policy stays
  byte-identical; the device-fleet cohort mechanism is recorded as a
  separate, separately-argued mechanism in P18.06 and P18.16.** Evidence:
  `crates/flexcore-kernel/src/revocation.rs` git diff vs HEAD EMPTY
  (close record 43 §4.5); record 06 quotes the policy verbatim and argues
  the new mechanism separately; record 42 compiles it; the P18.14 suite
  adds no rollout path. Status: **satisfied by the byte-identity check**.
- [x] **Criterion 6 — updates are specified as REFUSED while any command
  is ISSUED/TRACKING on the device (P18.15); no update path bypasses the
  command lifecycle or the authorization gates.** Evidence: the interlock
  (record 28) blocks every NON-TERMINAL frozen state — a recorded strict
  superset of the citation — and fails closed on an unknown device state;
  the two-step composition feeds the unchanged wave-1 session rule; the
  interlock is OBSERVER-only toward the command lifecycle (pinned by
  B's test) and one gate among several (the P18.13 attestation gate, the
  P18.06 orchestrator, the untouched P9.34 surface). Status: **satisfied
  by the interlock and the P18.14 concurrent-command battery**.
- [x] **Criterion 7 — no production signing key exists; test-grade signing
  stays labeled test-grade; every key ceremony is a human gate (P10.21
  interplay).** Evidence: the fixtures use the loudly-labelled
  `fnv1a64-test` verifier or the consumed P11.15 SHA-256 — no key material
  anywhere; record 03's key ceremonies are recorded as human gates;
  `Sha256UpdateVerifier` wiring is a DEPLOYMENT REQUIREMENT. Status:
  **satisfied by the labeled surfaces**.
- [x] **Criterion 8 — `python tools/verify_docs.py` and `python
  tools/validate_agent.py` are green after every documentation change in
  this phase (results quoted, never asserted).** Evidence: the verbatim
  outputs in the wave-1 close-outs (08, 27) and the final close run
  (close record 43 §5) — all green (EXIT=0 ×3, dependency checker
  included). Status: **satisfied, quoted**.

## Subtask coverage (16/16 with decision records)

| Subtask | Record | Status |
|---|---|---|
| P18.01 | 01 (+ P18-E-1 registered OPEN) | record-only (ADR) |
| P18.02 | 02 (+ `update_bundle.rs` + in-module tests) | written-unrun |
| P18.03 | 03 | record-only (repository plan) |
| P18.04 | 04 (+ `update_session.rs` flow) | written-unrun |
| P18.05 | 05 (+ the extended gate + the SHA-256 option) | written-unrun |
| P18.06 | 06 (+ `rollout_orchestrator.rs`; P18-E-3's DEFAULT implemented as configuration) | written-unrun |
| P18.07 | 07 (+ `update_evidence.rs`) | written-unrun |
| P18.08 | 21 (+ `identity_lifecycle.rs`) | written-unrun |
| P18.09 | 22 (+ `zero_touch.rs`) | written-unrun |
| P18.10 | 23 | record-only (runbook) |
| P18.11 | 24 (+ `esim_lifecycle.rs`) | written-unrun |
| P18.12 | 25 (+ migration 0023 + `fleet_registry.rs`) | written-unrun |
| P18.13 | 26 (+ `attestation.rs`; P18-E-2 registered OPEN) | written-unrun |
| P18.14 | 41 (+ the mock device + 4 test suites, 65 tests; F-1/F-2 resolved) | test-double depth, written-unrun |
| P18.15 | 28 (+ `update_command_interlock.rs` + B's integration file) + close-out 29 | written-unrun |
| P18.16 | 42 (decision register) + 43 (close + consistency audit) | register compiled; E-decisions OPEN; close + audit |

## Human-gate queue (NOT-STARTED / EMPTY / UNSIGNED)

1. First workspace human build + clippy + `cargo test --workspace` (covers
   every written-unrun suite, including the P18.14 batteries and the
   interlock suite). F-1 is resolved; any further failure is a recorded
   edit, never a silent reinterpretation.
2. The UNVERIFIED verification pass over the phase's registers (records
   01/02/03/04/21/22/24: TUF role/rotation semantics, the strictly-ahead
   sequence comparison, SUIT alignments, the OCA-text enumeration,
   802.1AR/FDO, SGP.22/SGP.32) — a mismatch is a recorded edit.
3. P18-E-1/E-2/E-3 review and closure by the human owner (record 42 §2
   names what closes each; every closer input is EMPTY / NOT-STARTED).
4. The P6.34 pilot hardware spec — fills the attestation assumption set
   (P18-E-2's closing condition; PENDING PILOT HARDWARE SPEC).
5. All key ceremonies (P10.21) and the repository operational commitment
   (R-P18-4) — nothing exists; no repository is stood up.
6. X-8 pre-registration: any future rollout run registers its canary size
   and abort thresholds BEFORE the run (structural in P18.06; the owner's
   recording of P18-E-3's values is still human input).
7. Orchestrator item F-4: the unescaped pipe in record 24's observation
   row (close record 43 §4.2).
