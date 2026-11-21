# W1.06 — Assessment aggregates and records

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 assessment aggregates and records from the design §4.2 table, with their key content and invariants, so later persistence and API work has one authoritative record list.
- **Design trace:** Design §4.2
- **Inputs / prerequisites:** Design §4.2; W1.05 domain model.
- **Steps:**
  1. Reproduce each §4.2 record with its key content and important invariant: Facility, Building, EnergyBoundary, MeterPoint, UtilityBill, MeterDataset, EquipmentItem, InspectionObservation, EnergyConservationMeasure (ECM), MeasureScenario, AssessmentRun, FlexibilityCandidate, QualificationDecision, AssetOnboardingDraft, EfficiencySavingsVerification.
  2. Reproduce EVERY invariant in the §4.2 table for each record — no invariant may be dropped: Facility (tenant derived structurally; sensitive address fields have restricted access); Building (each value carries source/provenance and units; missing values remain missing); EnergyBoundary (defines what consumption and savings mean; multiple meters allowed; no assumption that a whole-facility bill maps to one Core Site); MeterPoint (raw account identifiers are not exposed in normal DTOs/logs); UtilityBill (a billing-period financial record, not a telemetry row; currency explicit); MeterDataset (raw and normalized forms remain linked; DST ambiguity, duplicate/missing intervals and data gaps explicit); EquipmentItem (inspection record does not imply FlexCore asset qualification); InspectionObservation (user-declared, assessor-observed and instrument-measured evidence are distinct); EnergyConservationMeasure (catalog entry versioned and sourced; applicability checked; no universal savings percentage); MeasureScenario (immutable once run; interacting ECMs are not naively summed); AssessmentRun (immutable calculation record; a re-run creates a new ID); FlexibilityCandidate (candidate lifecycle is separate from Core asset lifecycle and dispatch authority); QualificationDecision (human sign-off and expiry are explicit); AssetOnboardingDraft (draft has no control effect; only an authorized approval creates/links Core asset/capability records); EfficiencySavingsVerification (separate from P16 demand-response event M&V; method and evidence are versioned).
  3. Record that tenant is derived structurally for Facility, and that sensitive address fields and raw account identifiers have restricted access and are not exposed in normal DTOs/logs.
  4. Cross-reference the records that later subtasks own in detail (ECM catalog W2.01, scenarios W2.02, candidates W2.03, EfficiencySavingsVerification W3.01) rather than duplicating their definitions here.
  5. Record the QualificationDecision fields in full from the §4.2 table: reviewer, evidence snapshot, rule-set version, accepted/rejected/conditional result, scope/expiry, reason.
- **Deliverable / output:** An aggregates-and-records note covering all 15 §4.2 records with key content and invariants.
- **Acceptance criteria / exit check:**
  - [ ] All 15 §4.2 records appear with key content and EVERY invariant in the §4.2 table (no invariant dropped).
  - [ ] The UtilityBill-is-not-telemetry, MeasureScenario-immutable, AssessmentRun-immutable and draft-has-no-control-effect invariants are stated.
  - [ ] The QualificationDecision fields are listed in full (reviewer, evidence snapshot, rule-set version, accepted/rejected/conditional result, scope/expiry, reason).
  - [ ] Restricted-access handling for sensitive address fields and raw account identifiers is stated.
  - [ ] Later-owned records are cross-referenced to their owning subtasks, not redefined.
- **Owner role:** Domain architecture
- **Dependencies:** W1.05
- **Verification method:** Table-by-table comparison against design §4.2; cross-reference check (no record defined twice).
- **Risks / failure modes:** Two subtasks defining the same record differently. Mitigation: this note is the single record list; later subtasks cross-reference it.
