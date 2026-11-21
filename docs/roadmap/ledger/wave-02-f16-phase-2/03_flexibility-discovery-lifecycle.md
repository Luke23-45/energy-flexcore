# W2.03 — Flexibility discovery lifecycle

Workstream: FlexCore control-plane work

- **Objective:** Define the flexibility discovery lifecycle from design §5.5: DISCOVERED, EVIDENCE_INCOMPLETE, ENGINEERING_REVIEW, QUALIFIED or REJECTED, ONBOARDING_DRAFT, APPROVED_AND_ONBOARDED, plus EXPIRED/SUSPENDED.
- **Design trace:** Design §5.5
- **Inputs / prerequisites:** Design §5.5; W1.06 FlexibilityCandidate record; W1.08 assessment lifecycle.
- **Steps:**
  1. Reproduce the candidate lifecycle as a transition table: DISCOVERED to EVIDENCE_INCOMPLETE to ENGINEERING_REVIEW to QUALIFIED or REJECTED to ONBOARDING_DRAFT to APPROVED_AND_ONBOARDED, with EXPIRED or SUSPENDED when the qualification evidence or operating configuration is no longer valid.
  2. State that discovery is a staged evidence process, not a boolean field on EquipmentItem.
  3. State that the candidate lifecycle is separate from the Core asset lifecycle and from dispatch authority.
  4. Record the lifecycle as the SINGLE test oracle for the qualification state-transition tests in W2.08 (W1.17 defers qualification state transitions to the Wave 2 test record — see W1.17 step 1): no other lifecycle oracles qualification transitions.
  5. Record the mapping to the §5.1 assessment-level states, so the two lifecycles never disagree: §5.5 ENGINEERING_REVIEW corresponds to §5.1 FlexibilityReview; §5.5 QUALIFIED corresponds to §5.1 CandidateQualified; §5.5 REJECTED corresponds to §5.1 CandidateRejected; §5.5 ONBOARDING_DRAFT corresponds to §5.1 OnboardingDraft; §5.5 APPROVED_AND_ONBOARDED corresponds to §5.1 HumanApproval and CoreAssetCreated. The mapping is partial by design: §5.5 DISCOVERED and EVIDENCE_INCOMPLETE occur before §5.1 FlexibilityReview, and §5.5 EXPIRED/SUSPENDED have no §5.1 counterpart. Where the two lifecycles appear to disagree, this §5.5 lifecycle governs qualification transitions. This step is the single definition of the §5.5-to-§5.1 state mapping; W1.08 step 5 references it and does not restate it.
- **Deliverable / output:** A flexibility discovery lifecycle note with the transition table, the staged-evidence rule, and the single §5.5-to-§5.1 state mapping.
- **Acceptance criteria / exit check:**
  - [ ] Every §5.5 lifecycle state appears, including EXPIRED/SUSPENDED.
  - [ ] Every transition in the §5.5 sequence appears; no shortcut transition is added.
  - [ ] Discovery as a staged evidence process (not a boolean field) is stated.
  - [ ] Separation from the Core asset lifecycle and dispatch authority is stated.
  - [ ] The §5.5-to-§5.1 state mapping is recorded, including its partial scope, as the single definition (W1.08 step 5 references it; no second copy).
  - [ ] This lifecycle is stated as the SINGLE test oracle for qualification state transitions; no other lifecycle oracles them.
- **Owner role:** Power systems
- **Dependencies:** W1.06, W1.08
- **Verification method:** Transition-table diff against design §5.5.
- **Risks / failure modes:** A candidate marked qualified by a single inspection field. Mitigation: qualification requires the staged lifecycle and the W2.04 checklist.
