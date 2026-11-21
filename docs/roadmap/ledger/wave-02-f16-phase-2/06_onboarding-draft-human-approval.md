# W2.06 — Onboarding draft and human approval

Workstream: FlexCore control-plane work

- **Objective:** Define the onboarding draft and human approval flow from design §5.6: a draft has no control effect; an authorized operator approves it; the Core registry creates the Asset/CapabilityModel through existing use cases; qualification does not grant dispatch authority.
- **Design trace:** Design §5.6; §6.2; §6.3; §13.5
- **Inputs / prerequisites:** Design §5.6; W2.05 mapping; W1.12 API surface (asset-onboarding-drafts routes); W1.04 roles; W1.06 QualificationDecision record.
- **Steps:**
  1. State the draft rule: an onboarding draft is only a proposal and has no control effect.
  2. Define the approval flow: an authorized operator approves the draft (asset_onboarding.approve); the Core registry creates the Asset and CapabilityModel through existing use cases and transactional boundaries.
  3. State that candidate qualification does not grant dispatch authority, and that dispatch authority remains governed by the Core command lifecycle and control-authority modes.
  4. Record the approval record contents (approver, evidence snapshot, scope, expiry) and the link to the QualificationDecision record from W1.06.
  5. Apply §13.5 to qualification and approval outputs: every QualificationDecision and approval record must state what data was used and absent, what is measured/declared/provider-derived/assumed, which tariff/weather/equipment model versions were used, the uncertainty and limitations, and whether more data or professional inspection is required. Two §13.5 items are not applicable here and must be recorded as such, not omitted silently: solver status (no solver runs in qualification or approval — the decision is a human approval of evidence, not a computed result) and whether any financial conclusion is unsupported (these outputs publish no financial conclusion — measure economics live in W2.02).
- **Deliverable / output:** An onboarding-draft and approval note with the draft rule, the approval flow and the no-dispatch-authority rule.
- **Acceptance criteria / exit check:**
  - [ ] The draft-has-no-control-effect rule is stated.
  - [ ] The approval flow (authorized operator approves; Core registry creates Asset/CapabilityModel through existing use cases) is stated.
  - [ ] Qualification-does-not-grant-dispatch-authority is stated.
  - [ ] The approval record contents are listed.
  - [ ] The §13.5 disclosure items are required on every QualificationDecision and approval record.
- **Owner role:** Product + power systems + security
- **Dependencies:** W2.03, W2.04, W2.05, W1.04, W1.06, W1.12
- **Verification method:** Flow checked against design §5.6 and §6.2; approval action checked against the W1.04 authorization matrix.
- **Risks / failure modes:** A draft treated as an asset, or qualification treated as dispatch permission. Mitigation: both rules are stated as contract and tested in the Wave 2 exit-gate review (W2.08).
