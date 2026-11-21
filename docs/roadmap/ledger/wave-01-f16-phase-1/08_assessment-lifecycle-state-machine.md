# W1.08 — Assessment lifecycle state machine

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 assessment lifecycle state machine from design §5.1, including the Incomplete and RevisionRequired loops, and the rule that an assessment can be issued without flexibility candidates.
- **Design trace:** Design §5.1
- **Inputs / prerequisites:** Design §5.1; W1.05, W1.06.
- **Steps:**
  1. Reproduce the §5.1 state diagram as a transition table: Draft, CollectingEvidence, DataValidation, Incomplete, ReadyForAnalysis, AnalysisRunning, AnalysisComplete, AssessorReview, RevisionRequired, AssessmentIssued, FlexibilityReview, CandidateQualified, CandidateRejected, OnboardingDraft, HumanApproval, CoreAssetCreated, with every transition shown in the diagram.
  2. State the loop rules: DataValidation goes to Incomplete on missing/invalid inputs and Incomplete returns to CollectingEvidence on corrected evidence; AssessorReview goes to RevisionRequired and RevisionRequired returns to CollectingEvidence.
  3. State that an assessment can be issued without any flexibility candidates (an energy report does not require a candidate to be dispatchable), and conversely that a resource can be inventoried before a complete whole-building efficiency study exists where the necessary equipment and telemetry evidence is available.
  4. Record the state machine as the test oracle for ASSESSMENT-level transitions only (Draft through AssessmentIssued and the FlexibilityReview entry); qualification state transitions are oracled by the §5.5 candidate lifecycle in W2.03, which is the SINGLE test oracle for qualification transitions (see step 5).
  5. Note that the mapping between the §5.1 assessment-level states and the §5.5 candidate lifecycle is defined once, in W2.03 step 5 (the §5.5 lifecycle owner), and is not restated here; this state machine oracles assessment-level transitions only (see step 4).
- **Deliverable / output:** An assessment lifecycle state-machine note: transition table plus the issue-without-candidates rule.
- **Acceptance criteria / exit check:**
  - [ ] Every §5.1 state appears in the transition table.
  - [ ] Every transition in the §5.1 diagram appears; no transition is added that the diagram does not show.
  - [ ] Both loop rules (Incomplete, RevisionRequired) are stated.
  - [ ] The issue-without-candidates rule and the inventory-before-complete-study rule are stated.
  - [ ] W2.03 step 5 is referenced as the single definition of the §5.1-to-§5.5 state mapping; no second copy of the mapping is recorded in this subtask.
  - [ ] The §5.5 candidate lifecycle (W2.03) is stated as the SINGLE test oracle for qualification transitions; this table oracles assessment-level transitions only.
- **Owner role:** Domain architecture
- **Dependencies:** W1.05, W1.06
- **Verification method:** Transition-table diff against the §5.1 diagram (each diagram edge appears once).
- **Risks / failure modes:** Adding a shortcut transition (for example Draft straight to AssessmentIssued) in a later phase. Mitigation: the table is the test oracle.
