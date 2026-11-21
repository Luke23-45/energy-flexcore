# W0.05 — Record the §0 executive decisions as the design baseline

Workstream: FlexCore control-plane work

- **Objective:** Record the design §0 executive decisions as the baseline for all later waves: F-16 as an in-repo bounded context, F-17 as a separate companion product, no generic shared-energy crate in advance, no second VPP optimizer, the operational audit log, DR event M&V and efficiency savings M&V kept as three separate concepts, and both features post-v1; and the four §0 product-boundary sentences.
- **Design trace:** Design §0; §2.3; §15 ADR-SHARED-01; foundations.md final recommendation (launch sequencing)
- **Owns:** Owns ADR-SHARED-01 (extract shared pure models only after a proven common calculation exists).
- **Inputs / prerequisites:** Design §0; §2.3 (no premature shared library); §15 ADR table.
- **Steps:**
  1. Transcribe each §0 executive decision verbatim in substance into the Wave 0 baseline record, with its design-doc row cited.
  2. For each decision, record what it forbids (for example: no generic shared-energy crate in advance; no second VPP optimizer; F-17 is not a FlexCore control-plane workspace).
  3. Record ADR-SHARED-01 as owned by this subtask: shared pure models are extracted only after a proven common calculation exists, with no persistence, identity, tenant context, network access, credentials or dispatch authority in any shared code; name the owners (architecture review) and the status (proposed, pending approval).
  4. Record the four §0 product-boundary sentences in the baseline record: "An assessment is not an asset. A discovered flexible load is not a qualified dispatch resource. A solar estimate is not an interconnection approval. An optimizer result is not authorization to control a device."
  5. Circulate the baseline record to product and architecture reviewers for acknowledgement.
  6. Record the launch-sequencing rule from the foundations.md final recommendation: F-16 is the proposed next FlexCore extension and comes first; F-17 stays a research candidate until its Phase 0 prototype demonstrates real demand; do not launch both simultaneously.
- **Deliverable / output:** A Wave 0 baseline record stating each §0 executive decision, what it forbids, the four §0 product-boundary sentences, and the ADR-SHARED-01 ownership.
- **Acceptance criteria / exit check:**
  - [ ] Every §0 executive-decision row appears in the baseline record with its design citation.
  - [ ] The three audit/M&V concepts are named separately (operational append-only audit log, DR event M&V, efficiency savings M&V) with no conflation.
  - [ ] The four §0 product-boundary sentences appear in the baseline record in substance (assessment is not an asset; a discovered flexible load is not a qualified dispatch resource; a solar estimate is not an interconnection approval; an optimizer result is not authorization to control a device).
  - [ ] ADR-SHARED-01 is recorded as owned here, with its extraction rule stated.
  - [ ] Owners (architecture review) and status are recorded.
  - [ ] The record states both features are post-v1 and outside the v1.0.0 release gate.
  - [ ] The launch-sequencing rule is stated (F-16 first; F-17 stays a research candidate until its Phase 0 prototype demonstrates real demand; do not launch both simultaneously).
- **Owner role:** Product + architecture
- **Dependencies:** W0.01
- **Verification method:** Line-by-line comparison of the baseline record against design §0 and §15.
- **Risks / failure modes:** A later wave quietly creates a shared crate or a second optimizer. Mitigation: the baseline record is the scope-creep test at every later wave gate.
