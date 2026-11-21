# W5.11 — §8.2 nine-step user journey trace record

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Own the design §8.2 nine-step user journey end to end as a trace record: reproduce all nine steps with the step-5 equipment/constraints rule and the step-8 decision-package contents, and trace each step to its implementing subtask instead of duplicating that work. Added in review iteration 01 (finding 1-09): design §8.2 had no owning subtask. This subtask is numbered after the Phase 1 scope review (W5.10) because review-added subtasks continue the sequence and existing IDs are never renumbered; it is worked after W5.02/W5.03/W5.05/W5.06 and before the scope review evidences it.
- **Design trace:** Design §8.2; foundations.md §3 item 5 (step-8 decision-package contents)
- **Inputs / prerequisites:** Design §8.2; W5.02 (steps 1–5); W5.03 (step 6); W5.05 (steps 7–8 report); W5.06 (step 9).
- **Steps:**
  1. Reproduce the §8.2 nine-step journey in order: (1) property and location; (2) electricity usage; (3) tariff; (4) roof; (5) equipment and constraints; (6) compare scenarios; (7) review outputs; (8) decision package; (9) optional sharing.
  2. Record the step-5 rule: select candidate PV module/inverter assumptions and battery options; keep generic assumptions visibly separate from actual vendor quotations.
  3. Record the step-8 decision-package contents, from foundations.md §3 item 5 (A decision and implementation report) — design §8.2 step 8 itself requires only a downloadable report able to recommend "do not install" or "collect more data"; the seven contents below are the foundations §3 item 5 list: whether solar makes economic sense; which configuration is most appropriate; what assumptions drive the result; what information is still missing; what financial risks exist; whether a professional site inspection is necessary; what the household needs to do to proceed. The package must be comfortable recommending that a household not install solar when the economics or physical conditions do not support it.
  4. Trace each step to its implementing subtask (steps 1–5 → W5.02; step 6 → W5.03; steps 7–8 → W5.05; step 9 → W5.06) and record any step with no implementing subtask as a gap, not as covered.
- **Deliverable / output:** A §8.2 journey trace record with all nine steps, the step-5 rule, the step-8 contents, and the per-step implementing-subtask trace.
- **Acceptance criteria / exit check:**
  - [ ] All nine §8.2 steps are reproduced in order.
  - [ ] The step-5 equipment/constraints rule (generic assumptions visibly separate from vendor quotations) is stated.
  - [ ] The step-8 decision-package contents are listed, including the comfortable "do not install" recommendation.
  - [ ] Every step is traced to an implementing subtask or recorded as a gap.
- **Owner role:** Product
- **Dependencies:** W5.02, W5.03, W5.05, W5.06
- **Verification method:** Journey checked step by step against design §8.2; trace checked against the implementing subtasks.
- **Risks / failure modes:** Journey steps implemented in isolation with no end-to-end trace, so step 5 or step 8 is silently dropped. Mitigation: the trace record is a Phase 1 scope-review input (W5.10).
