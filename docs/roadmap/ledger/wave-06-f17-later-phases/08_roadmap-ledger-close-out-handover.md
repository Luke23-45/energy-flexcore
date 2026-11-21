# W6.08 — Roadmap ledger close-out and handover

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Close out the roadmap ledger: verify every wave's exit gate (where design §14 defines one) and scope review is evidenced, and produce the F-16/F-17 status report as the handover.
- **Design trace:** Design §14 (all waves)
- **Inputs / prerequisites:** W0.08, W1.18, W2.08, W4.13 exit-gate records, W3.05 and W5.10 scope-review records; W6.01–W6.07 outputs.
- **Steps:**
  1. Verify each wave exit gate in turn (Gate 0, F-16 Phase 1, Phase 2, F-17 Phase 0) against its review record, and verify the F-16 Phase 3 scope review (W3.05) and the F-17 Phase 1 scope review (W5.10) — design §14 defines no exit gate for either.
  2. Verify the counts reconcile (76 subtasks across Waves 0–6) and that every subtask ID is unique and contiguous within its wave.
  3. Produce the F-16/F-17 status report: per-wave status, per-gate status (G1–G9), per-ADR status (12 ADRs), open items, and the conditions for any later work.
  4. Hand over the complete ledger (MASTER_LEDGER.md, README.md, FINALIZATION.md, all wave files) with the status report as the closing record.
- **Deliverable / output:** A roadmap ledger close-out record with the exit-gate verification (and the F-16 Phase 3 and F-17 Phase 1 scope reviews), the count reconciliation, the F-16/F-17 status report, and the handover.
- **Acceptance criteria / exit check:**
  - [ ] Every wave exit gate (Gate 0, F-16 Phase 1, Phase 2, F-17 Phase 0) has a verdict with its review record cited, and the F-16 Phase 3 scope review (W3.05) and the F-17 Phase 1 scope review (W5.10) are evidenced.
  - [ ] The count reconciles to 76 subtasks with unique, contiguous IDs per wave.
  - [ ] The status report covers per-wave, per-gate (G1–G9) and per-ADR (12 ADRs) status plus open items.
  - [ ] The handover names the complete ledger file set.
- **Owner role:** Product + platform engineering
- **Dependencies:** W0.08, W1.18, W2.08, W3.05, W4.13, W5.10, W6.01, W6.02, W6.03, W6.04, W6.05, W6.06, W6.07
- **Verification method:** Exit-gate records inspected; counts re-grepped; status report checked against gate and ADR tables.
- **Risks / failure modes:** Closing the ledger while a wave gate is unevidenced. Mitigation: each gate needs its review record cited.
