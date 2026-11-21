# W1.07 — Multidimensional data provenance

Workstream: FlexCore control-plane work

- **Objective:** Define multidimensional data provenance for F-16: source kind, source reference, measurement quality, model uncertainty, verification state and time validity as independent dimensions, with the unknown-narrows rule; and define the immutable calculation-record and determinism rules for F-16 analysis jobs.
- **Design trace:** Design §4.3; §13.1; §13.2; §15 ADR-DATA-01
- **Owns:** Owns ADR-DATA-01 (source, quality, uncertainty and verification are independent provenance dimensions). Also covers §13.1 immutable calculation records for F-16 and §13.2 determinism for F-16 analysis jobs.
- **Inputs / prerequisites:** Design §4.3, §13.1, §13.2; design §1.1 data-quality and calibrated-model-records rows; W1.06 records.
- **Steps:**
  1. Define the six provenance dimensions from §4.3 (source kind; source reference; measurement quality as one of the six canonical states where measurement-like; model uncertainty as a range/distribution or explicit unknown; verification state; time validity) and state they must not be collapsed into a single confidence or quality badge. Enumerate the §4.3 value lists in full: source kinds (utility-issued statement, calibrated meter, BMS export, equipment nameplate, assessor observation, owner declaration, public weather feed, commercial data provider, model-derived value) and verification states (unreviewed, source-validated, assessor-reviewed, engineering-reviewed, expired, rejected).
  2. State the unknown-narrows rule: unknown constraints must not widen a feasible range or earn positive dispatch credit (preserving the existing P3.02 rule).
  3. Define the §13.1 immutable calculation record contents for F-16 (input snapshot ID and digest, raw-data/document digests, parser/importer version, source/provider and API version, weather dataset or station ID/distance/quality, tariff profile ID/version and effective date, ECM catalog or model version, optimizer/solver version with termination status and gap where relevant, assumptions and overrides, completeness and exclusions, uncertainty/limitations and warnings, produced-at time and code build/version).
  4. Define the §13.2 determinism rules for F-16 jobs (existing worker envelope with request ID, idempotency key, schema version, snapshot ID, model/worker version and determinism metadata; tolerances and solver status recorded where a provider is not bitwise deterministic; no exact-replay claim the engine cannot provide; provider request parameters and response digest stored with credentials removed; model evaluation datasets and hand-calculated reference cases kept separate from live user data).
  5. Record ADR-DATA-01 as owned by this subtask. Name the owners (data architecture, safety review) and the status (proposed, pending approval).
- **Deliverable / output:** A provenance note covering the six dimensions, the unknown-narrows rule, the immutable-record contents and the determinism rules, with ADR-DATA-01.
- **Acceptance criteria / exit check:**
  - [ ] All six §4.3 provenance dimensions are defined as independent; no single-badge collapse is permitted.
  - [ ] The §4.3 enumerations are listed in full (the nine source kinds; the six verification states).
  - [ ] The unknown-narrows rule is stated with its P3.02 preservation noted.
  - [ ] The §13.1 immutable-record contents are listed in full for F-16.
  - [ ] The §13.2 determinism rules are stated, including the no-exact-replay-claim rule and the separation of model evaluation datasets and hand-calculated reference cases from live user data.
  - [ ] ADR-DATA-01 is recorded as owned here.
  - [ ] Owners (data architecture, safety review) and status are recorded.
- **Owner role:** Data architecture + safety review
- **Dependencies:** W1.05, W1.06
- **Verification method:** Comparison against design §4.3, §13.1 and §13.2; ADR-DATA-01 reviewed by data architecture and safety review.
- **Risks / failure modes:** A quality flag presented as a statistical confidence interval, or an unknown constraint earning flexibility credit. Mitigation: the dimensions are independent by ADR and the unknown-narrows rule is explicit.
