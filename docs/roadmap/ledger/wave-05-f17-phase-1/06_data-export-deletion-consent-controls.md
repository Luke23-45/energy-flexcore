# W5.06 — Data export, deletion and consent controls

Workstream: companion-repo work (separate identity/database — NOT FlexCore control-plane work)

- **Objective:** Define F-17 data export, deletion and consent controls from design §11.3: separate household database/keys/bucket, PII minimization, no logging of account numbers/addresses/tokens, quarantine and scan uploads, short-lived URLs, encryption, and defined deletion/export/backup-expiry.
- **Design trace:** Design §14 F-17 Phase 1; §11.3
- **Inputs / prerequisites:** Design §11.3; W4.11 G8 decisions; W5.01 separate data store.
- **Steps:**
  1. Reproduce the §11.3 minimization rules: separate household database, service credentials, encryption keys and object-store bucket from FlexCore; store precise postal address apart from calculation-ready coordinates where feasible; only send minimum required location precision to providers; never log utility-account numbers, raw addresses, bill contents, access tokens or signed URLs; upload documents to quarantine, scan them and parse in a resource-limited worker; use short-lived upload/download URLs, CSRF protection, rate limits, IDOR protection and explicit owner authorization; encrypt data at rest and in transit; define deletion, export and backup-expiry behaviour. These four authorization controls (CSRF protection, rate limits, IDOR protection, explicit owner authorization) are defined here; W5.07 applies them to every route by reference and does not redefine them.
  2. Define the export control (what a homeowner can export, in what form, and what is excluded) and the deletion workflow (start documented deletion subject to legal retention requirements, per W4.11).
  3. State the default: do not use household bills, locations or occupancy data for model training or marketing by default.
  4. State the consent rule: require a separate, specific, revocable consent before sharing a report or interval data with an installer, utility or aggregator; keep an access/sharing record; consent revocation stops future access and deletes/withdraws derived copies where contractually and technically possible.
- **Deliverable / output:** A data export, deletion and consent controls note with the §11.3 rules, the export/deletion definitions, and the consent rule.
- **Acceptance criteria / exit check:**
  - [ ] The separate database/keys/bucket and the PII-minimization rules are stated.
  - [ ] The no-logging rule (account numbers, addresses, bill contents, tokens, signed URLs) is stated.
  - [ ] Quarantine/scan uploads, short-lived URLs, encryption, and defined deletion/export/backup-expiry are stated.
  - [ ] The four API-authorization controls (explicit owner authorization, IDOR protection, rate limits, CSRF protection) are defined here.
  - [ ] The no-training/marketing-by-default rule and the specific revocable consent rule are stated.
- **Owner role:** Privacy/security + product
- **Dependencies:** W4.11, W5.01
- **Verification method:** Controls checked against design §11.3 and the W4.11 G8 record.
- **Risks / failure modes:** Logging a raw address or an access token, or sharing without specific consent. Mitigation: both are forbidden in the note and tested in W5.09.
