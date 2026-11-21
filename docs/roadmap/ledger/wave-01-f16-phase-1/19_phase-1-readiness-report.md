# W1.19 — Phase 1 readiness report

Workstream: FlexCore control-plane work

- **Objective:** Produce the Phase 1 readiness report from design §14 F-16 Phase 1: data completeness, missing evidence, interval quality and boundaries. Added in the adversarial-audit patch pass (2026-11-21): the §14 readiness report had no owning subtask in Waves 0–3. This subtask is numbered after the Phase 1 exit-gate review (W1.18) because review-added subtasks continue the sequence and existing IDs are never renumbered; in execution order it is worked before the exit-gate review (W1.18) evidences it.
- **Design trace:** Design §14 F-16 Phase 1
- **Inputs / prerequisites:** Design §14 F-16 Phase 1; W1.09 ingestion; W1.10 baseline diagnostics.
- **Steps:**
  1. Report data completeness: which required evidence is present and which is missing, per assessment, with the missing items named (not a completeness percentage alone).
  2. Report missing evidence as a named list with the assessment it blocks, not as an implicit gap.
  3. Report interval quality: meter coverage, missing/duplicate intervals, quality states and data gaps, per MeterDataset, from the W1.09/W1.10 diagnostics.
  4. Report boundaries: the EnergyBoundary(s) the report covers, the meters mapped to each, and any bill or meter that could not be mapped to a boundary.
  5. State the readiness verdict: whether the evidence package is ready for analysis, or what must be collected first.
- **Deliverable / output:** A readiness report per assessment: completeness, named missing evidence, interval quality, boundaries, and the readiness verdict.
- **Acceptance criteria / exit check:**
  - [ ] Data completeness is reported with missing evidence named per assessment.
  - [ ] Interval quality (coverage, missing/duplicate intervals, quality states, gaps) is reported per MeterDataset.
  - [ ] The covered EnergyBoundary(s) and any unmapped bill or meter are stated.
  - [ ] The readiness verdict (ready for analysis, or what must be collected first) is stated.
- **Owner role:** Product + power systems
- **Dependencies:** W1.09, W1.10
- **Verification method:** Report checked against design §14 F-16 Phase 1; completeness and interval figures traced to the W1.09/W1.10 diagnostics.
- **Risks / failure modes:** Reporting a completeness percentage that hides which evidence is missing. Mitigation: missing evidence is a named list.
