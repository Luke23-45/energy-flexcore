# Phase 9 — Customer Pilot Qualification: Exit-Gate Checklist

**Exit gate (§23 Stage 9): customer/site and applicable authorities approve the deployment
and its control scope.**

Each item is a checkbox plus the evidence required. Check an item only when the evidence
exists at the location named and has been inspected. A checked item without evidence is not
checked. Partial approval (read-only/shadow only) does not close this checklist; record it
as a reduced control scope and keep the remaining items open.

## Qualification evidence

- [ ] **Pilot scope frozen** — `pilot-scope-v1.0` signed/acknowledged by customer and Neryva
  product ownership (P9.01). Evidence: the signed scope document naming customer, sites,
  assets, service, control scope, and exclusions.
- [ ] **Registry reconciled** — zero unresolved discrepancies for pilot assets; capability
  profiles complete (P9.02). Evidence: reconciliation log + corrected registry extract.
- [ ] **Network model validated** — versioned, frozen grid model per site with validation
  residuals and written coverage limitations (P9.03). Evidence: model + validation report.
- [ ] **Integration design complete** — every adapter named with version/profile, mappings,
  and conformance suite; no uncertified conformance claim (P9.04). Evidence: integration
  design document.
- [ ] **Site network design approved** — zone/flow diagram, outbound-only, allowlist, customer
  network/OT owner review (P9.05). Evidence: design + review record.
- [ ] **D6 closed** — OT/IT boundary review signed by security and the customer OT owner,
  per boundary decision recorded (P9.06). Evidence: D6 closure record. **Gate: no read-only
  pilot activity before this item.**
- [ ] **Security assessment complete** — every §17.4 threat with a control and evidence;
  no must-fix finding open (P9.07). Evidence: assessment report + findings list.
- [ ] **Identity and access enforced** — role matrix, IdP integration, step-up actions,
  negative tests passed (P9.08). Evidence: role matrix + negative-test audit extracts.
- [ ] **Secrets and certificates managed** — inventory, no secret in source control or plugin
  config, rotation tested, revocation measured (P9.09). Evidence: inventory + rotation test
  record + scan result.
- [ ] **Data governance defined** — hosting/residency, per-class retention, tenancy test,
  log inspection, export/deletion procedure (P9.10). Evidence: data-governance note.
- [ ] **Market/program requirements captured** — program rules from program documentation;
  no universal qualification claim (P9.11). Evidence: requirements note + mapping.
- [ ] **Authorities engaged** — every applicable authority listed with status (P9.12).
  Evidence: engagement tracker with submission references.

## Testing evidence

- [ ] **Commissioning plan ready** — per-site steps with pass/fail criteria, safety rules,
  rollback (P9.13). Evidence: commissioning plan reviewed by the customer site contact.
- [ ] **Lab integration passed** — read-only end-to-end, command path demonstrably disabled,
  fault injection recorded (P9.14). Evidence: lab test report with versions/seeds.
- [ ] **HIL passed** — CHIL/PHIL scope stated, IEEE 1547.1-2020 cases where applicable,
  failures dispositioned, pre-gates evidenced (P9.15). Evidence: HIL test report.
- [ ] **Field acceptance plan agreed** — cases with pre-set thresholds, frozen versions,
  failure process, customer agreement (P9.16). Evidence: agreed test plan.
- [ ] **Field acceptance executed, shadow first** — shadow cases passed/waived, shadow review
  before promotion, operator-approved cases only after shadow acceptance (P9.17). Evidence:
  execution record per site.
- [ ] **Telemetry verified** — quality mapping, clock sync within bound, freshness met,
  stale/missing injection holds/rejects (P9.18). Evidence: telemetry verification report.
- [ ] **Command lifecycle dry-run passed** — each gate failed independently at least once;
  no command reached ISSUED in shadow (P9.19). Evidence: dry-run report + audit extracts.
- [ ] **Fallback drills passed** — per-asset/site policies reviewed with the site operator;
  every degraded mode drilled; no drill failure open (P9.20). Evidence: fallback register
  + drill report.

- [ ] **Forged/replayed/expired/out-of-sequence commands rejected at the edge** — Evidence: attack-test record; zero attempts reached an adapter or device. (P9.31)
- [ ] **Audit integrity and edge audit buffer proven** — Evidence: tamper-detection record + outage upload test (zero loss, zero duplication). (P9.32)
- [ ] **Edge enrollment ceremony tested** — Evidence: ceremony record; spent/expired token reuse refused; compromise re-enrollment tested. (P9.33)
- [ ] **Signed edge updates verified; rollback tested** — Evidence: bad-signature refusal + failed-update rollback record. (P9.34)
- [ ] **§17.5 safety invariants mapped to pilot tests** — Evidence: invariant→test map with all ten invariants and live tests. (P9.35)
- [ ] **Live incident drill executed** — Evidence: live drill record (detection to containment, notification vs deadline, reversion to shadow/local). (P9.36)
- [ ] **Adapter conformance suites executed in the pilot configuration** — Evidence: per-adapter suite execution report against pilot versions; failures dispositioned. (P9.37)
- [ ] **Pilot shadow replay reproduces within tolerance** — Evidence: shadow-replay record for a full operating day (or longest continuous shadow run). (P9.38)
- [ ] **Pilot performance/load test passed** — Evidence: load-test report at pilot scale (ingest p95/p99, scheduling runtime, fan-out, recovery, degraded case). (P9.39)
- [ ] **HIL-to-field traceability complete** — Evidence: traceability map; every FAT case has upstream evidence or a signed accepted gap. (P9.40)
- [ ] **Scenario-regression suite re-run on the pilot configuration** — Evidence: run record with suite version; failures dispositioned. (P9.41)

## Operational readiness evidence

- [ ] **Operators qualified** — every approver passed the practical exercise; no approval
  right held by an unqualified person (P9.21). Evidence: qualification register.
- [ ] **Operational procedures written** — per in-scope use case, walked against a shadow
  case (P9.22). Evidence: pilot operations manual (versioned).
- [ ] **Incident response ready** — incident classes, escalation, notification deadlines,
  tabletop exercised with the customer, rota current (P9.23). Evidence: procedure +
  tabletop record.
- [ ] **Backup/restore rehearsed** — restore actually performed and verified; measured time
  and loss window recorded; edge outage test passed (P9.24). Evidence: rehearsal report.
- [ ] **Observability verified** — correlation fields present, critical alerts triggered and
  received, logs clean (P9.25). Evidence: verification report + alert catalogue.

## Approval evidence

- [ ] **Control scope documented and enforced** — deployed configuration matches the document;
  no plugin can raise the mode; no automation envelope present (P9.26). Evidence:
  control-scope document + configuration verification record.
- [ ] **Limitations and risks registered** — customer-acknowledged, every item owned and
  dated, no certification/safety wording (P9.27). Evidence: register.
- [ ] **Customer/site acceptance recorded** — review held, decision per site, conditions owned
  and dated, waivers signed and time-bounded (P9.28). Evidence: acceptance review record.
- [ ] **Authority approvals recorded** — every required approval decided or tracked with a
  follow-up date; no site operates beyond its granted approvals (P9.29). Evidence: approval
  pack + decision tracker.
- [ ] **Exit-gate record complete** — gate decision recorded, checklist complete, handover
  pack indexed with frozen versions (P9.30). Evidence: exit-gate record.
