# W0.01 — Reconcile the AGENTS.md docs-only contradiction

Workstream: FlexCore control-plane work

- **Objective:** Reconcile the contradiction between AGENTS.md (which says the repository is documentation-only with no application code) and the actual branch state (README plus the source tree showing written-unrun code), before this design becomes authoritative, so no agent relies on the stale statement or silently chooses which repository state is authoritative.
- **Design trace:** Design §1.2
- **Inputs / prerequisites:** Design §1.2; root README.md; root AGENTS.md; current source tree (Cargo workspace members).
- **Steps:**
  1. Compare AGENTS.md §1 against the README and the source tree, and record each conflicting statement verbatim with its file and line.
  2. Determine the true branch state (workspace members present, code written-unrun) from the tree itself, not from either document alone.
  3. Draft the corrected AGENTS.md wording that states the actual state and the document hierarchy, and record it as a proposed edit for the repository owners (this ledger does not edit AGENTS.md itself).
  4. Record which statement agents must treat as authoritative until the correction lands, and where that instruction lives.
- **Deliverable / output:** A reconciliation note in the Wave 0 record: the conflicting statements quoted, the verified branch state, the proposed AGENTS.md correction text, and the interim authority instruction.
- **Acceptance criteria / exit check:**
  - [ ] The note quotes each conflicting AGENTS.md / README / source-tree statement with file and line.
  - [ ] The note states the verified branch state (written-unrun code present) and names the evidence used.
  - [ ] The proposed AGENTS.md correction text is recorded as a proposal, not silently applied.
  - [ ] The interim authority instruction names one source agents must follow until the correction lands.
- **Owner role:** Product + platform engineering
- **Dependencies:** None (first subtask; Gate 0 entry).
- **Verification method:** Line-by-line comparison of AGENTS.md §1 against README.md and the source tree; the proposed correction is reviewed against design §1.2.
- **Risks / failure modes:** An agent builds on the stale docs-only statement and scaffolds or omits work on a false premise. Mitigation: the interim authority instruction is explicit and the correction is a named Gate 0 exit item.
