# W5.12 — Record the final G7 decision (REopt API v3 vs local REopt.jl)

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Record the FINAL G7 decision (REopt API v3 vs local REopt.jl) per design §10.4 step 6 and §16 gate G7, after the model is understood and user demand is validated. Added in the adversarial-audit patch pass (2026-11-21): W5.03 recorded the final decision inside the subtask that runs scenarios through the engine, before the §10.4 step 6 conditions were met. This subtask is numbered after the Phase 1 scope review (W5.10) because review-added subtasks continue the sequence and existing IDs are never renumbered; in execution order it is worked after W5.03 and before the scope review (W5.10) evidences it.
- **Design trace:** Design §10.4 step 6; §16 gate G7
- **Owns:** Owns the FINAL §16 gate G7 decision (the interim position is W4.10).
- **Inputs / prerequisites:** Design §10.4 step 6; §16 item 7; W4.10 interim position; W4.06 demand validation; W5.03 scenario experience; W4.05 benchmark record.
- **Steps:**
  1. Confirm the §10.4 step 6 trigger is met: the model is understood (W5.03 scenario work and W4.05 benchmark record) and user demand is validated (W4.06 usability record). If either is not met, record the decision as still deferred with owner, venue and date; do not decide it.
  2. Decide REopt API v3 vs local REopt.jl on the G7 dimensions (API terms, quotas and data sharing vs local operational burden, Julia runtime, solver licensing and maintenance), consuming the W4.10 interim position and stating what changed since it.
  3. Record the decision as a build-vs-buy/license/operations decision, not a foregone conclusion, with owner, venue and date.
  4. File the decision in the decision register and state its consequences for the scenario engine used in W5.03.
- **Deliverable / output:** A final G7 decision record: the decision, its §10.4 step 6 trigger evidence, the W4.10 interim position it consumes, and its consequences for the scenario engine.
- **Acceptance criteria / exit check:**
  - [ ] The §10.4 step 6 trigger (model understood, demand validated) is evidenced from W5.03/W4.05 and W4.06, or the decision is recorded as still deferred with owner, venue and date.
  - [ ] The decision is recorded on the G7 dimensions, consuming the W4.10 interim position and stating what changed since it.
  - [ ] The decision is filed with owner, venue and date, and its consequences for the W5.03 scenario engine are stated.
- **Owner role:** Numerical engineering + license/operations review
- **Dependencies:** W5.03, W4.05, W4.06, W4.10
- **Verification method:** Decision record checked against design §10.4 step 6 and §16 item 7; trigger evidence inspected.
- **Risks / failure modes:** Deciding the engine before the model is understood or demand is validated. Mitigation: the trigger is an acceptance item, and the decision stays deferred until it is met.
