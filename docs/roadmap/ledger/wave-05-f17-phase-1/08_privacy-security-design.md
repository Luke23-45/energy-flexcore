# W5.08 — Privacy and security design: §11.3 control-coverage verification matrix

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Verify the F-17 privacy and security design from design §11.3 as a control-coverage matrix: every §11.3 control is traced to the subtask that defines it (W5.06 for data controls and the four API-authorization controls; W5.07 for the route table and per-route application) and to its verification, with no control defined twice and none uncovered. Rewritten in the adversarial-audit patch pass (2026-11-21): the previous version reproduced §11.3 content already defined in W5.06/W5.07 without unique checkable content. This subtask defines no control itself; it verifies coverage.
- **Design trace:** Design §11.3
- **Inputs / prerequisites:** Design §11.3; W5.06 controls; W5.07 API surface.
- **Steps:**
  1. Build the coverage matrix: one row per §11.3 control (separate household database, service credentials, encryption keys and object-store bucket; precise postal address stored apart from calculation-ready coordinates; minimum location precision sent to providers; never log utility-account numbers, raw addresses, bill contents, access tokens or signed URLs; quarantine/scan uploads and parse in a resource-limited worker; short-lived upload/download URLs; CSRF protection; rate limits; IDOR protection; explicit owner authorization; encryption at rest and in transit; defined deletion, export and backup-expiry behaviour; no household data for model training or marketing by default; separate, specific, revocable consent before sharing; access/sharing record with revocation stopping future access; no direct path to FlexCore dispatch, commands, credentials or site-edge connections). Each row names the single defining subtask (W5.06 for data controls and the four API-authorization controls — explicit owner authorization, IDOR protection, rate limits, CSRF protection; W5.07 for the route table and per-route application) and the verification (W5.09 test item or design review).
  2. Verify completeness: every §11.3 control appears exactly once as a definition in W5.06/W5.07; any control with no defining subtask is recorded as a gap, not as covered.
  3. Verify no duplication: no §11.3 control is defined as a normative rule in more than one subtask; this matrix cross-references definitions, it does not restate them.
  4. Verify the boundary row: F-17 has no direct path to FlexCore dispatch, commands, credentials or site-edge connections (design §11.3, ADR-F17-01).
- **Deliverable / output:** A §11.3 control-coverage matrix with every control traced to its defining subtask and verification, plus the no-dispatch-path boundary verification.
- **Acceptance criteria / exit check:**
  - [ ] Every §11.3 control appears in the matrix with its defining subtask (W5.06 or W5.07) and its verification named.
  - [ ] No §11.3 control is left without a defining subtask; any gap is recorded as a gap.
  - [ ] No §11.3 control is defined as a normative rule in more than one subtask.
  - [ ] The no-path-to-FlexCore-dispatch/commands/credentials rule is verified against ADR-F17-01.
- **Owner role:** Security + privacy
- **Dependencies:** W5.06, W5.07, W4.12
- **Verification method:** Matrix checked row by row against design §11.3; definitions checked against W5.06/W5.07 (no restated rules); boundary checked against ADR-F17-01 (W4.12).
- **Risks / failure modes:** A §11.3 control with no owner, or the same control defined differently in two subtasks. Mitigation: the matrix is the single coverage record, and W5.09 tests the controls it traces.
