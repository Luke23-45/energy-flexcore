# W0.02 — Execute and resolve the existing workspace, console, static, security and verification gates

Workstream: FlexCore control-plane work

- **Objective:** Execute and resolve the existing workspace, console, static, security and verification gates recorded in the README and the v1 ledger, and record the findings, before sizing or starting any F-16/F-17 extension work.
- **Design trace:** Design §1.3; §14 Gate 0 item 2
- **Inputs / prerequisites:** Design §1.3; README release/verification status; v1 ledger gates (ledger/MASTER_LEDGER.md, FINALIZATION.md); repo check scripts tools/verify_docs.py and tools/validate_agent.py.
- **Steps:**
  1. Run the existing workspace build, console, static, security and verification checks as the repository requires, and record actual outputs (not summaries) per gate.
  2. Triage every failure or open gate: record what failed, the evidence, and whether it blocks extension sizing.
  3. Record the resolved state of each gate, or the named owner and follow-up where a gate cannot be closed in Wave 0.
  4. Confirm in writing that no F-16/F-17 sizing in later waves assumes a gate result that was not actually obtained.
- **Deliverable / output:** A Wave 0 gate-resolution record listing each existing gate, its actual check output, its resolved or open status, and its owner where still open.
- **Acceptance criteria / exit check:**
  - [ ] Every gate named in design §1.3 has an entry with an actual check output or a recorded reason it could not be run.
  - [ ] No gate is recorded as passed without evidence attached.
  - [ ] Any gate still open has a named owner and a follow-up, and later waves do not assume its result.
  - [ ] The record states explicitly whether extension sizing may proceed on the resolved state.
- **Owner role:** Platform engineering + security
- **Dependencies:** W0.01
- **Verification method:** Re-run of the repo checks with outputs quoted; cross-check against the v1 ledger gate list.
- **Risks / failure modes:** Treating a written-unrun workspace as verified. Mitigation: only actual check outputs count as evidence; verbal agreement is not evidence.
