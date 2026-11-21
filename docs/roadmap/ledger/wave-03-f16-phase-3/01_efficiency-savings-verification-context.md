# W3.01 — EfficiencySavingsVerification context

Workstream: FlexCore control-plane work

- **Objective:** Define the EfficiencySavingsVerification context from design §7.3: boundary, baseline/reporting periods, meter coverage, adjustments, M&V plan version, exclusions/uncertainty, reviewer/report status; separate from P16 DR event M&V.
- **Design trace:** Design §7.3; §4.2
- **Inputs / prerequisites:** Design §7.3; W1.06 EfficiencySavingsVerification record; design §1.1 DR baseline/M&V row (P16.04/P16.05).
- **Steps:**
  1. Define the context contents from §7.3: the affected system or whole-facility measurement boundary; baseline and reporting periods; meter coverage and fuel types; weather, production, occupancy and operating-condition adjustments; non-routine adjustment records; M&V plan version and chosen method; data exclusions and uncertainty; independent reviewer and report status.
  2. State the separation explicitly: this is the post-retrofit efficiency savings workflow with a different boundary and purpose from the existing P16.04/P16.05 demand-response program baselines and event delivery workflow; the two must not share a record type or a result.
  3. State the naming discipline: this context is efficiency savings M&V; the operational append-only audit log and DR event M&V are separate concepts and must not be conflated with it.
  4. Cross-reference W3.02 as the workflow that operates on this context.
- **Deliverable / output:** An EfficiencySavingsVerification context note with the §7.3 contents and the separation from P16 DR event M&V.
- **Acceptance criteria / exit check:**
  - [ ] All §7.3 context contents appear (boundary, periods, meter coverage, adjustments, M&V plan version/method, exclusions/uncertainty, reviewer/report status).
  - [ ] Separation from P16.04/P16.05 DR event M&V is stated, with the different boundary and purpose.
  - [ ] The three-concepts naming discipline is stated (operational audit log, DR event M&V, efficiency savings M&V).
  - [ ] W3.02 is cross-referenced as the workflow owner.
- **Owner role:** M&V practitioner + product
- **Dependencies:** W1.06, W2.08
- **Verification method:** Context checked against design §7.3 and §4.2; separation checked against the P16.04/P16.05 design rows.
- **Risks / failure modes:** Merging efficiency savings results into a DR event report, or vice versa. Mitigation: separate context, separate record type.
