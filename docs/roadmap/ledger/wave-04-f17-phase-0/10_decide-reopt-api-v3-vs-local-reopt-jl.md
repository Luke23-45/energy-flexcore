# W4.10 — Decide REopt API v3 vs local REopt.jl

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Record the REopt API v3 versus local REopt.jl comparison and an interim position only, per design §16 G7 and §10.4 step 6: the Phase 0 prototype uses the REopt API v3 proof-of-concept adapter; the final decision is deferred until the model is understood and user demand is validated.
- **Design trace:** Design §16 gate G7; §10.4
- **Owns:** Owns §16 gate G7 — interim position only (REopt API vs local REopt.jl: API terms/quotas/data sharing vs local operational burden, Julia runtime, solver licensing/maintenance; the final decision is W5.12).
- **Inputs / prerequisites:** Design §16 item 7; §10.4; W0.03 D4 record (REopt license/terms).
- **Steps:**
  1. Compare the two options on the G7 dimensions: API terms, quotas and data sharing (REopt API v3, stable API, REopt.jl engine) versus local operational burden, Julia runtime, solver licensing and maintenance (REopt.jl source, Apache 2.0 license file, with all transitive dependencies, solvers and API terms still to be reviewed before it becomes a production dependency).
  2. Record an INTERIM position only: the Phase 0 prototype uses the REopt API v3 proof-of-concept adapter (§10.4: the leading candidate for an external optimization reference and proof-of-concept adapter); the final API-vs-local decision is deferred until the model is understood and user demand is validated (W4.06), per §10.4 step 6. Record owner, venue and date for the deferred final decision.
  3. State that the decision is a build-vs-buy/license/operations decision, not a foregone conclusion, and record the revisit trigger (model understood and user demand validated, per §10.4 step 6).
  4. File the G7 decision or interim position in the decision register.
- **Deliverable / output:** A G7 decision record: the REopt API v3 vs local REopt.jl comparison, the interim position, the prototype choice (REopt API v3 proof-of-concept adapter), and the deferred-decision trigger.
- **Acceptance criteria / exit check:**
  - [ ] Both options are compared on the G7 dimensions (terms/quotas/data sharing vs operational burden, Julia runtime, solver licensing/maintenance).
  - [ ] An interim position (not a final decision) is filed with owner, venue and date for the deferred final decision.
  - [ ] The prototype uses the REopt API v3 proof-of-concept adapter, with its reason recorded.
  - [ ] The revisit trigger (model understood and user demand validated) is recorded.
- **Owner role:** Numerical engineering + license/operations review
- **Dependencies:** W4.05, W4.06, W0.03
- **Verification method:** G7 record checked against design §16 item 7 and §10.4; REopt references cited from design §17.
- **Risks / failure modes:** Treating the hosted API as decided without reviewing terms/quotas/data sharing, or the local package without reviewing transitive dependencies. Mitigation: both reviews are required before a production dependency.
