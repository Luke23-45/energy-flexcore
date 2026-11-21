# W1.12 — API surface and events/async work

Workstream: FlexCore control-plane work

- **Objective:** Define the F-16 API surface from design §6.2 and the events and asynchronous-work contract from §6.3: tenant-derived operations, explicit action authorization, unit-of-work execution, outbox events, and worker results as proposals/data, never commands.
- **Design trace:** Design §6.2; §6.3
- **Inputs / prerequisites:** Design §6.2, §6.3; W1.04 roles; W1.11 placement.
- **Steps:**
  1. Reproduce the §6.2 routes table as method+path strings with purpose per route: `POST /api/v1/facility-assessments` (create draft assessment and bind facility/scope); `GET /api/v1/facility-assessments/{id}` (read assessment status and summary); `POST /api/v1/facility-assessments/{id}/uploads` (create evidence upload intent); `POST /api/v1/facility-assessments/{id}/bills` (register parsed/reviewed utility bill record); `POST /api/v1/facility-assessments/{id}/meter-imports` (start interval data import job); `POST /api/v1/facility-assessments/{id}/buildingsync-imports` (validate and stage BuildingSync XML); `POST /api/v1/facility-assessments/{id}/equipment-observations` (record or update a versioned inspection observation); `POST /api/v1/facility-assessments/{id}/analysis-runs` (start immutable baseline/measure analysis); `GET /api/v1/facility-assessments/{id}/analysis-runs/{run_id}` (read result, warnings and provenance); `POST /api/v1/facility-assessments/{id}/flexibility-candidates/{candidate_id}/review` (record human qualification review); `POST /api/v1/facility-assessments/{id}/asset-onboarding-drafts` (propose mapping to existing Core Site/Asset); `POST /api/v1/asset-onboarding-drafts/{id}/approve` (authorized approval through Core use cases).
  2. State the API rules: all tenant-owned operations derive the tenant from authenticated TenantContext, authorize an explicit action, and execute inside a unit of work; cross-tenant reads return indistinguishable not-found outcomes; upload and analysis are idempotent and bounded; use the proposed explicit action grants (assessment.create, assessment.read, assessment.evidence.upload, assessment.run, flexibility_candidate.review, asset_onboarding.propose, asset_onboarding.approve) as proposed names, not existing grants.
  3. Reproduce the §6.3 outbox event examples (FacilityAssessmentCreated; AssessmentEvidenceAccepted/AssessmentEvidenceRejected; MeterDatasetImported/MeterDatasetImportFailed; AssessmentAnalysisCompleted; MeasureScenarioEvaluated; FlexibilityCandidateDiscovered; FlexibilityQualificationRecorded; AssetOnboardingDraftApproved) and the transactional outbox discipline.
  4. Define the long-running analysis request envelope (request ID, idempotency key, input snapshot ID/digest, schema version, engine version, model/provider version, declared determinism metadata) and the worker result envelope (result plus diagnostics), and state the worker returns a result envelope and diagnostics and cannot create or issue a dispatch command.
- **Deliverable / output:** An API-surface and events note with the §6.2 routes, the API rules, the §6.3 events, and the worker-envelope contract.
- **Acceptance criteria / exit check:**
  - [ ] All 12 §6.2 routes appear as method+path strings with their stated purpose.
  - [ ] The tenant-from-TenantContext, explicit-action, unit-of-work and indistinguishable-not-found rules are stated.
  - [ ] All §6.3 example events appear with the transactional outbox discipline.
  - [ ] The worker-envelope contents are listed and the worker-cannot-command rule is stated.
- **Owner role:** Platform engineering
- **Dependencies:** W1.04, W1.11
- **Verification method:** Route-by-route comparison against design §6.2 and §6.3; action-grant names checked against W1.04.
- **Risks / failure modes:** A worker result treated as a dispatch command, or a cross-tenant read leaking existence. Mitigation: the envelope and not-found rules are stated as contract, not guidance.
