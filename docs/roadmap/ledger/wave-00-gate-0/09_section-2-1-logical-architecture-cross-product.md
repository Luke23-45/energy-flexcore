# W0.09 — Record the §2.1 logical architecture as the cross-product deployment diagram

Workstream: FlexCore control-plane work

- **Objective:** Record the design §2.1 logical architecture as the cross-product deployment diagram, including the external provider seams and the rule that the two products meet only at the §12 enrollment boundary. Added in review iteration 01 (finding 1-08): design §2.1 had no owning subtask. This subtask is numbered after the Gate 0 exit-gate review (W0.08) because review-added subtasks continue the sequence and existing IDs are never renumbered; in execution order it is worked before Wave 1, and the Gate 0 exit-gate review (W0.08) evidences it.
- **Design trace:** Design §2.1; §12 enrollment boundary
- **Inputs / prerequisites:** Design §2.1; W0.05 baseline record.
- **Steps:**
  1. Reproduce the §2.1 diagram in substance: Assessment Studio → existing FlexCore control-plane API → F-16 bounded context → tenant-scoped PostgreSQL, evidence object storage, and workers → existing Core registry, telemetry, flexibility, scheduling and validation via a reviewed onboarding package; and separately, the F-17 homeowner web application → household API → separate consumer database, separate encrypted file store, and planning/simulation worker.
  2. Record the external provider seams: external weather/solar resource providers → F-16 workers and the F-17 calculation worker; optional rooftop geometry provider → household API; jurisdiction tariff and cost packs → F-16 and the household API; REopt / PVWatts / SAM reference engines → the F-17 calculation worker.
  3. State the boundary rule: the two products meet only at the §12 enrollment boundary (signed, versioned ResidentialDEREnrollmentPackage, explicit opt-in, aggregator review); there is no shared persistence, shared identity, or unified UI.
  4. Record that W1.11 (F-16 placement) cross-references this record for its placement in the joint diagram. The Wave 5 cross-reference is owned by W5.01 (step 5) in Wave 5, not claimed here.
- **Deliverable / output:** A §2.1 logical architecture record with the cross-product diagram, the external provider seams, and the §12-only meeting-point rule.
- **Acceptance criteria / exit check:**
  - [ ] The §2.1 diagram appears with both products and their separate stores.
  - [ ] All four external provider seams are recorded with their consumers.
  - [ ] The §12-only meeting-point rule (no shared persistence, identity or UI) is stated.
  - [ ] W1.11 cross-references this record (the W5.01 cross-reference is owned by W5.01 in Wave 5).
- **Owner role:** Architecture
- **Dependencies:** W0.05
- **Verification method:** Diagram and seams checked against design §2.1 item by item.
- **Risks / failure modes:** The two products drifting into shared persistence or a unified UI. Mitigation: the §12-only rule is recorded in the Wave 0 baseline.
