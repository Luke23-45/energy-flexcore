# W5.07 — API surface

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define the F-17 API surface from the design §11.2 routes table (home-plans, property, usage-imports, roof-assessments, tariff-options, scenarios, scenario-runs, share-consents, deletion workflow).
- **Design trace:** Design §11.2
- **Inputs / prerequisites:** Design §11.2; W5.01 scope; W5.02 model; W5.06 controls.
- **Steps:**
  1. Reproduce the §11.2 routes table as method+path strings with purpose per route: `POST /api/v1/home-plans` (create a homeowner project); `PUT /api/v1/home-plans/{id}/property` (save property and coarse/precise location with consent); `POST /api/v1/home-plans/{id}/usage-imports` (upload bill/interval data or create a Green Button consent flow where supported); `POST /api/v1/home-plans/{id}/roof-assessments` (request provider roof data or submit manual roof planes); `GET /api/v1/tariff-options` (search available reviewed jurisdiction tariff profiles); `POST /api/v1/home-plans/{id}/scenarios` (save a versioned PV/battery alternative); `POST /api/v1/home-plans/{id}/scenario-runs` (queue an immutable analysis run); `GET /api/v1/home-plans/{id}/scenario-runs/{run_id}` (retrieve outputs, model versions and caveats); `POST /api/v1/home-plans/{id}/share-consents` (grant specific data sharing to a named recipient); `DELETE /api/v1/home-plans/{id}/share-consents/{consent_id}` (revoke future sharing access); `DELETE /api/v1/home-plans/{id}` (start the documented deletion workflow subject to legal retention requirements).
  2. Apply the §11.3 authorization controls defined in W5.06 to every route: the controls (explicit owner authorization, IDOR protection, rate limits, CSRF protection) are defined in W5.06, not restated here; this subtask records only that every route is subject to them.
  3. State that the API is the independent household API from W5.01, not a FlexCore control-plane route, and that no route reaches FlexCore dispatch, commands, credentials or site-edge connections.
- **Deliverable / output:** An F-17 API surface note with the §11.2 routes table and the per-route application of the §11.3 authorization controls defined in W5.06.
- **Acceptance criteria / exit check:**
  - [ ] All 11 §11.2 routes appear as method+path strings with their stated purpose.
  - [ ] Every route is subject to the §11.3 authorization controls defined in W5.06.
  - [ ] The API is stated as the independent household API, not a FlexCore control-plane route.
  - [ ] No route to FlexCore dispatch/commands/credentials/site-edge connections is permitted.
- **Owner role:** Platform engineering
- **Dependencies:** W5.01, W5.02, W5.06
- **Verification method:** Routes checked against design §11.2; per-route application checked against the W5.06 definitions.
- **Risks / failure modes:** A route that accepts another homeowner ID and returns their plan. Mitigation: the W5.06 authorization controls apply to every route and are tested in W5.09.
