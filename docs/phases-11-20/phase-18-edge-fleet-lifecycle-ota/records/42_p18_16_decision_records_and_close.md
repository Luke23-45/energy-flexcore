# Record 42 — P18.16 (decision-register half): P18-E-1, P18-E-2, and P18-E-3 verified OPEN with DEFAULTs — none closed by any agent

Date: 2026-11-17. Subtask: P18.16, decision-register half (phase-18 plan §2
row P18.16; conventions §4 — **agents never close an E-decision**). Status:
REGISTER COMPILATION (record-only depth). Every entry below was verified in
BOTH register locations at the close date by direct inspection; nothing was
edited, nothing closed. **No update bundle has ever been produced, signed,
staged, or applied; no device exists; no fleet exists — nothing in this
register compiles a runtime claim of any kind.**

## 1. The verified register

| ID | Where registered (both locations verified on disk) | Options (recorded in plan §6) | DEFAULT (label) | What was IMPLEMENTED under the DEFAULT | Closer (human) | Status |
|---|---|---|---|---|---|---|
| P18-E-1 | plan §6 row 1; `ledger_phase_18.md` §Decision points row 1; registered OPEN with DEFAULT (b) in `records/01_p18_01_ota_architecture_adr.md` (the ADR; record header names the registration) | (a) full TUF repository from day one; (b) minimal signed manifest with TUF-shaped role separation designed-in, full repository above a recorded fleet threshold; (c) plain signed bundles (rejected — no key-compromise recovery path) | **(b)** | the v2 manifest format (record 02) is the minimal signed manifest with role separation designed-in; `UPDATE_BUNDLE_STATEMENT` pins that no TUF-conformance claim is made and no repository is operated | the owner recording the fleet-size threshold and the operational commitment to repository expiry/rotation discipline (R-P18-4) | **OPEN** |
| P18-E-2 | plan §6 row 2; `ledger_phase_18.md` §Decision points row 2; registered OPEN with DEFAULT (a) in `records/26_p18_13_attestation_design.md` §1 (its own registration table) | (a) hardware-optional — attestation degrades to software evidence, the difference stated honestly per case; (b) hardware-required (rejected for 0.x); (c) ignore hardware (rejected — dishonest) | **(a)** | `attestation.rs` implements the with/without table verbatim (`HARDWARE_POSTURE_STATEMENT`: every hardware determination PENDING PILOT HARDWARE SPEC, the P6.34 posture; software-only evidence is consistency checking that CANNOT detect a compromised reporter); the P18.14 attestation battery pins both the default and the named anchor-required refusal | the pilot hardware spec (P6.34 cells) filling the determination | **OPEN** |
| P18-E-3 | plan §6 row 3 (the phase decision register — registered WITH the rollout cohort policy defaults, per the wave plan); `ledger_phase_18.md` §Decision points row 3; the implemented-DEFAULT dependence is recorded in `records/06_p18_06_rollout_orchestrator_scope_distinction.md` §4 | (a) canary 5–10% → progressively larger blocks, failure-rate abort thresholds pre-registered; (b) operator-manual per-block approval only; (c) full-fleet simultaneous (rejected — no staged safety evidence) | **(a)** | `rollout_orchestrator.rs` implements the DEFAULT as configuration-shaped mechanism: the canary band 500..=1000 bp is a REQUIRED pre-registered plan parameter (X-8, structural — `CanarySizeOutOfRange` refuses 499/1001), the 5–10% band is a DEFAULT RECOMMENDATION not a sourced figure, the abort threshold is required at plan time and evaluated per-outcome with every evaluation audited; the mechanism is SEPARATE from the kernel plugin-rollout policy (record 06, the kernel policy quoted verbatim and byte-identical) | the owner recording the thresholds after reviewing the P18.06 design | **OPEN** |

No re-registration was needed for P18-E-3: the wave plan's register (plan
§6 + the ledger decision table) already carries it with the rollout cohort
policy defaults, and record 06 §4 records the implemented-DEFAULT
dependence. This record compiles all three; it closes none.

## 2. What would close each (the recorded closing conditions, restated)

| ID | Closing evidence required | Current closer input |
|---|---|---|
| P18-E-1 | the recorded fleet-size threshold + the operational commitment (expiry/rotation discipline; R-P18-4) | EMPTY / NOT-STARTED |
| P18-E-2 | the pilot hardware spec filling the P6.34 determination cells | EMPTY / PENDING PILOT HARDWARE SPEC |
| P18-E-3 | the owner recording canary size and abort thresholds after reviewing the P18.06 design | EMPTY / NOT-STARTED (any future run pre-registers its values first — X-8) |

## 3. Interlocks the register depends on (cited, not re-argued)

- The kernel plugin-rollout policy stays cohort/percentage-free BY
  CONSTRUCTION and byte-identical (`crates/flexcore-kernel/src/revocation.rs`
  — git diff vs HEAD empty at close); P18-E-3's DEFAULT creates a SEPARATE,
  separately-argued device-fleet mechanism (record 06).
- The P18.06 pre-registration discipline (X-8) is STRUCTURAL: no plan can
  be built without the canary band and the thresholds, so no run can
  precede the registration (the phase-17 close-record pattern).
- The P18-E-2 assumption set is the same P6.34 posture the P9.34/P18.05
  surfaces carry; P18.14's tests pin the honesty statements, not hardware
  behavior.

Full close + consistency audit: `records/43_phase_18_close_and_consistency_audit.md`.
