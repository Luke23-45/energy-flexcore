# Phase 9 — Customer Pilot Qualification: Ledger

**Phase:** 9 (Stage 9, report §23)
**Release target:** v1.0.0 pilot-ready (operator-approved control by default; §2.1, §21.4)
**Status:** Not started. No subtask is complete until its acceptance criteria are met and the
evidence named in the subtask exists and has been checked.
**Research cut-off:** 7 November 2026 (report §27). Nothing in this ledger invents a version,
date, or certification status after that cut-off.

> **Scope note.** FlexCore is not certified, not a safety case, and not permission for live
> grid control. This phase qualifies **one named customer deployment at named sites** for
> operator-approved control. It does not certify FlexCore, any adapter, any inverter, or any
> site. It does not grant bounded automation: bounded automation is post-1.0, per deployment,
> site, and asset class, after separate qualification (§21.4, §25). A subtask that cannot be
> evidenced is open, not passed.

## How to read this ledger

- Every subtask has an ID (`P9.01` … `P9.41`), a report trace (`§N` / `§N.M`), inputs,
  detailed steps, a concrete deliverable, testable acceptance criteria, an owner role,
  dependencies, a verification method, and risks/failure modes.
- Checkbox `- [ ]` marks an incomplete subtask. Check it only when the acceptance criteria
  are met and the evidence is filed in the location the subtask names.
- "Customer" means the named pilot customer. "Site" means a named pilot site. Where a
  subtask needs a customer- or site-specific value, the subtask says so; do not substitute a
  generic value and call it done.
- Canonical units throughout: MW, MVAr, MVA, MWh; `P > 0` = export at the point of
  connection; reactive-power sign declared per grid model and device profile; SOC as a
  fraction in [0,1] at the API boundary (§10.2). Telemetry quality is exactly one of
  `GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE` (§10.4). Use "license" (US
  spelling) and "acknowledgement" (per `.agent/WRITING.md`).
- Open gates carried into this phase: D1 (target jurisdiction and market), D2 (first
  portfolio and service), D3 (network class and model fidelity), D5 (first protocol and
  asset mix), D6 (cyber and OT boundary review). D6 must close **before any read-only
  pilot** (`.agent/DECISIONS.md`); D1–D3 and D5 must be closed for the deployment
  jurisdiction, network class, and protocol mix before the deployment is promoted beyond
  read-only mode (§22.3). D4 (license and dependency audit) was gated before v0.1.0; its
  final closure for the v1.0.0 release artifact set is a Phase 10 subtask (P10.19), not a
  Phase 9 subtask.

## Phase exit gate

**Exit gate (§23 Stage 9): customer/site and applicable authorities approve the deployment
and its control scope.** The gate is met only when every subtask in this ledger is checked
and the `CHECKLIST.md` in this directory is complete with the evidence it names. Partial
approval (for example, approval for read-only/shadow operation only) does **not** close
Phase 9; it is recorded as a reduced control scope and the remaining subtasks stay open.

## Subtask index

| ID | Title | Trace |
|----|-------|-------|
| P9.01 | Pilot scope, customer, and site freeze | §23 Stage 9, §25.2, §2.1 |
| P9.02 | Site and asset register reconciliation | §23 Stage 9, §10.1, §4 |
| P9.03 | Pilot network model build and validation | §23 Stage 9, §12.4, §12.5, §25.2 (D3) |
| P9.04 | Customer-specific integration design | §23 Stage 9, §2.3, §25.2 (D5) |
| P9.05 | Site network and connectivity design | §23 Stage 9, §3.3, §17.2, §18.3 |
| P9.06 | OT/IT boundary review — D6 closure | §23 Stage 9, §17, §25.2 (D6) |
| P9.07 | Deployment security assessment | §23 Stage 9, §17.1–§17.4 |
| P9.08 | Identity, access, and approval design | §23 Stage 9, §17.3, §20.2 |
| P9.09 | Secrets, certificates, and key management | §23 Stage 9, §17.3, §18.1 |
| P9.10 | Data residency, retention, and privacy design | §23 Stage 9, §16.2–§16.3, §25.2 (D1) |
| P9.11 | Market / program requirements capture | §23 Stage 9, §2.3, §15, §25.2 (D1) |
| P9.12 | Interconnection and authority engagement | §23 Stage 9, §22.3, §2.3 |
| P9.13 | Site commissioning plan | §23 Stage 9, §19.2, §21.2 |
| P9.14 | Lab integration test — read-only end-to-end | §23 Stage 9, §19.2, §23 Stage 6 |
| P9.15 | Hardware-in-the-loop test plan and execution | §23 Stage 9, §19.1–§19.2 |
| P9.16 | Field acceptance test plan | §23 Stage 9, §19.2, §22.3 |
| P9.17 | Field acceptance test execution — shadow first | §23 Stage 9, §19.2, §21.4 |
| P9.18 | Site telemetry quality and clock-sync verification | §23 Stage 9, §10.3–§10.4, §17.4 |
| P9.19 | Command-lifecycle dry run in shadow mode | §23 Stage 9, §14, §21.4 |
| P9.20 | Fallback and degraded-mode drills | §23 Stage 9, §14.4, §17.4, §21.2 |
| P9.21 | Operator training and role qualification | §23 Stage 9, §20, §17.3 |
| P9.22 | Market / product-specific operational procedures | §23 Stage 9, §1.5, §15 |
| P9.23 | Incident response and customer notification procedure | §23 Stage 9, §17.3–§17.4 |
| P9.24 | Backup, restore, and disaster-recovery rehearsal | §23 Stage 9, §17.3, §21.2 |
| P9.25 | Observability and alerting verification at the pilot | §23 Stage 9, §21.3 |
| P9.26 | Control-scope document and operator-approved configuration | §23 Stage 9, §21.4, §22.3 |
| P9.27 | Known-limitations and residual-risk register | §23 Stage 9, §17.4, §22.3 |
| P9.28 | Customer and site acceptance review | §23 Stage 9, §22.3 |
| P9.29 | Applicable-authority approval pack | §23 Stage 9, §22.3 |
| P9.30 | Phase 9 exit-gate record and handover to Phase 10 | §23 Stage 9, §22.3 |
| P9.31 | Forged/replayed/expired/out-of-sequence rejection at the edge | §17.4, §18.2 |
| P9.32 | Audit integrity and edge audit-buffer proof | §17.3, §18.1 |
| P9.33 | Edge enrollment ceremony and identity lifecycle | §18.1, §17.3 |
| P9.34 | Signed edge-update verification and rollback | §18.3, §17.3 |
| P9.35 | §17.5 safety-invariant map (pilot) | §17.5 |
| P9.36 | Live incident drill: forged-command containment | §17.3, §17.4 |
| P9.37 | Adapter conformance suites executed (pilot config) | §2.3, §19.2 |
| P9.38 | Pilot shadow replay (§24 step 10 at the pilot) | §24, §19.2 |
| P9.39 | Pilot performance and load test | §19.2, §19.4 |
| P9.40 | HIL-to-field traceability map | §19.1, §19.2 |
| P9.41 | Scenario-regression suite re-run (pilot config) | §22.2, §19.2 |
| P9.31 | Forged/replayed/expired/out-of-sequence rejection at the edge | §17.4, §18.2 |
| P9.32 | Audit integrity and edge audit-buffer proof | §17.3, §18.1 |
| P9.33 | Edge enrollment ceremony and identity lifecycle | §18.1, §17.3 |
| P9.34 | Signed edge-update verification and rollback | §18.3, §17.3 |
| P9.35 | §17.5 safety-invariant map (pilot) | §17.5 |
| P9.36 | Live incident drill: forged-command containment | §17.3, §17.4 |
| P9.37 | Adapter conformance suites executed (pilot config) | §2.3, §19.2 |
| P9.38 | Pilot shadow replay (§24 step 10 at the pilot) | §24, §19.2 |
| P9.39 | Pilot performance and load test | §19.2, §19.4 |
| P9.40 | HIL-to-field traceability map | §19.1, §19.2 |
| P9.41 | Scenario-regression suite re-run (pilot config) | §22.2, §19.2 |

---

## Subtasks

### P9.01 — Pilot scope, customer, and site freeze

- [ ] **P9.01 Pilot scope, customer, and site freeze**

- **Objective:** Freeze, in writing, exactly which customer, which sites, which assets, which
  service, and which control scope this pilot qualifies — so no later subtask can silently
  widen the pilot.
- **Report trace:** §23 Stage 9; §25.2 (D1, D2); §2.1 (v1.0.0 definition).
- **Inputs / prerequisites:** D1, D2, D3 decisions or their written interim positions; the
  customer agreement or letter of intent; the Phase 8 exit record.
- **Steps:**
  1. Record the customer legal name, the named pilot sites (site IDs per §10.1), and the
     asset inventory per site (asset IDs, types, vendors/models, rated power/energy).
  2. Record the service being piloted (from §1.5 first-generation use cases only; e.g.
     portfolio visibility, flexibility estimation, day-ahead/intraday scheduling, peak
     management, demand response) and the market/program, if any, per P9.11.
  3. Record the control scope in the terms of §21.4: read-only/shadow first, then
     operator-approved control; bounded automation explicitly excluded.
  4. Record what is out of scope: protection, primary frequency response, synthetic
     inertia, grid switching, hard-real-time control (§0.3, §1.6).
  5. Get the scope document signed (or otherwise formally acknowledged) by the customer
     and by Neryva product ownership.
- **Deliverable / output:** `pilot-scope-v1.0` document: customer, sites, assets, service,
  control scope, exclusions, signatories, date.
- **Acceptance criteria / exit check:** (a) every pilot site and asset is named by ID — no
  "and similar sites" language; (b) the service is one of the §1.5 first-generation use
  cases; (c) bounded automation is explicitly excluded in writing; (d) both parties have
  acknowledged the document.
- **Owner role:** Product (with legal for the customer agreement).
- **Dependencies:** None inside Phase 9 (entry subtask). Blocks P9.02–P9.30.
- **Verification method:** Document review against §1.5 and §21.4; confirm named IDs exist
  in the asset registry (P9.02 cross-check).
- **Risks / failure modes:** Scope creep mid-pilot (a second site or asset class added
  without re-qualification); D1/D2 still open, so the scope rests on an interim position —
  record that as an open risk in P9.27, not as a decision.

### P9.02 — Site and asset register reconciliation

- [ ] **P9.02 Site and asset register reconciliation**

- **Objective:** Prove the FlexCore registry matches physical reality at each pilot site
  before any schedule or command is built from it.
- **Report trace:** §23 Stage 9; §10.1; §4 (Asset Registry, Portfolio & Sites).
- **Inputs / prerequisites:** P9.01 scope document; customer asset lists, nameplates,
  single-line diagrams; site EMS inventory exports.
- **Steps:**
  1. For each pilot site, reconcile every asset in the registry against the customer's
     asset list and nameplate data: asset ID, type, vendor/model, rated power/energy,
     phase, bus reference, capability profile (§10.1).
  2. Reconcile site records: portfolio, location, timezone, PCC, grid-model reference,
     edge identity (§10.1).
  3. Record discrepancies in a reconciliation log with a disposition for each (corrected
     in registry / customer data wrong / needs site visit).
  4. Confirm capability-profile fields state which constraints are measured, configured,
     inferred, or unknown (§11.1).
  5. Re-run reconciliation after any correction and record the final clean pass.
- **Deliverable / output:** Reconciliation log + corrected registry extract per site,
  versioned and dated.
- **Acceptance criteria / exit check:** (a) zero unresolved discrepancies between registry
  and customer/site records for pilot assets; (b) every asset has a capability profile with
  measured/configured/inferred/unknown marked; (c) every site has a PCC and grid-model
  reference.
- **Owner role:** Integration engineering (with customer site contact).
- **Dependencies:** P9.01. Blocks P9.03, P9.04, P9.14.
- **Verification method:** Independent re-check of a sample of at least 20% of assets (or
  all assets if the pilot has fewer than 25) against nameplates/diagrams by a second
  person.
- **Risks / failure modes:** Stale customer asset lists; unrecorded replacements or
  retrofits; capability values copied from a datasheet that does not match the installed
  firmware — treat vendor data as unverified until the site confirms it.

### P9.03 — Pilot network model build and validation

- [ ] **P9.03 Pilot network model build and validation**

- **Objective:** Build a versioned grid model of each pilot feeder/site network that the
  feasibility gate (§12.5) can actually use, and validate it against known measurements.
- **Report trace:** §23 Stage 9; §12.4; §12.5; §25.2 (D3 — network class and model fidelity).
- **Inputs / prerequisites:** P9.02 reconciled registry; customer/DSO network data (feeder
  topology, conductor/transformer parameters, operating limits); D3 decision for the
  deployment (balanced radial, unbalanced three-phase, or other — do not assume).
- **Steps:**
  1. Record the network class chosen for the pilot and the D3 basis for it; if D3 is still
     open, record the interim class and the condition that would change it.
  2. Build the grid model (§10.1: buses, branches, transformers, switches, phases, base
     values, limits) with a model version, source-data references, and base values
     declared explicitly (§10.2).
  3. Select the grid-model provider per §12.4 for the declared model class (e.g.
     pandapower for balanced power flow; OpenDSS or GridLAB-D where unbalanced
     three-phase is required). Record why the provider fits the class; do not use a
     steady-state power-flow tool as evidence for transient stability (§19.1).
  4. Validate the model: run power flow against at least one recorded historical
     operating point per site (measured PCC power, voltages where available) and record
     residuals.
  5. Record model coverage limitations explicitly: what the model does not represent
     (unmodelled branches, estimated impedances, missing phase data).
  6. Freeze the model version used for pilot acceptance; later changes require a new
     version and re-validation.
- **Deliverable / output:** Versioned grid model per site + validation report (provider,
  class, residuals, coverage limitations, source-data references).
- **Acceptance criteria / exit check:** (a) model version is frozen and referenced by ID;
  (b) power-flow validation residuals against measured points are recorded and within the
  thresholds set for the pilot in advance (thresholds recorded before the run, not fitted
  after); (c) coverage limitations are written down, not omitted; (d) the provider matches
  the declared network class.
- **Owner role:** Power systems engineering.
- **Dependencies:** P9.02; D3 position. Blocks P9.14, P9.15, P9.19.
- **Verification method:** Independent re-run of the validation power flow from the frozen
  model and the recorded inputs; compare residuals.
- **Risks / failure modes:** Estimated impedances presented as measured; unbalanced
  network modelled as balanced; model validated only at one benign operating point and
  then trusted at reverse-power-flow extremes (cf. §12.5 SOCP-relaxation caution).

### P9.04 — Customer-specific integration design

- [ ] **P9.04 Customer-specific integration design**

- **Objective:** Design, for this customer, exactly how FlexCore connects to each site EMS,
  device, and protocol — adapter by adapter, with no adapter claimed conformant merely
  because it was implemented.
- **Report trace:** §23 Stage 9; §2.3; §5.3; §25.2 (D5 — first protocol and asset mix).
- **Inputs / prerequisites:** P9.01, P9.02; D5 decision; vendor documentation for the site
  EMS (e.g. OpenEMS) and each device/protocol in scope.
- **Steps:**
  1. List each integration point: site EMS, each protocol adapter (e.g. SunSpec Modbus,
     OpenADR 3.0, OCPP 2.0.1/2.1, IEEE 2030.5), and the direction of each flow
     (telemetry in, command out).
  2. For each adapter, record the protocol version, the profile/variant, the capability
     mapping (§5.3), and the conformance/interoperability test suite that applies
     (cf. §2.3: implementing an adapter is not a conformance claim).
  3. Record the adapter's read/write posture: read/telemetry first; write operations only
     in the explicit test and qualified profiles (§2.3).
  4. Define the mapping of vendor units, sign conventions, and status codes into the
     canonical contracts (§10.2, §6.4 anti-corruption layer), including the declared
     reactive-power sign per device profile.
  5. Define failure behaviour per adapter: timeout, partial response, stale data, and the
     telemetry quality state each maps to (§10.4).
  6. Record what is deliberately not integrated in this pilot.
- **Deliverable / output:** Integration design document per site: adapter list, versions,
  profiles, mappings, conformance suites, failure behaviour, exclusions.
- **Acceptance criteria / exit check:** (a) every integration point has a named adapter,
  version, and profile; (b) no adapter is described as "certified" or "conformant" without
  a cited test result (§17/Writing rules); (c) unit/sign mappings are recorded per device
  profile; (d) failure behaviour is defined for every adapter.
- **Owner role:** Integration engineering (with power systems for capability mapping).
- **Dependencies:** P9.01, P9.02; D5 position. Blocks P9.05, P9.14, P9.15.
- **Verification method:** Design review against §2.3 and §5.3; trace each adapter to its
  vendor documentation and its conformance suite.
- **Risks / failure modes:** Vendor firmware version differs from the documented profile;
  a protocol variant (e.g. OCPP 2.0.1 vs 2.1) assumed instead of confirmed with the fleet;
  write capability enabled by default in an adapter that was only qualified for reads.

### P9.05 — Site network and connectivity design

- [ ] **P9.05 Site network and connectivity design**

- **Objective:** Design the network path between each site and the central platform so that
  only documented flows cross each zone boundary, with no inbound internet access into OT
  networks.
- **Report trace:** §23 Stage 9; §3.3; §17.2; §18.3.
- **Inputs / prerequisites:** P9.04 integration design; customer network architecture and
  firewall policy; P9.06 boundary review (run in parallel; this design is its input).
- **Steps:**
  1. Draw the zone diagram for the pilot (§17.2: enterprise IT, central application,
     customer integration/DMZ, OT/site control, field/device) with every flow labelled
     (protocol, port, direction, initiator).
  2. Specify the edge connection as outbound-initiated from the customer edge to the
     central platform (§3.3), or through a customer-controlled broker/gateway where the
     customer requires it; record any DMZ, unidirectional gateway, or jump-host pattern
     the customer requires.
  3. Specify the allowlist: explicit source/destination/port per flow; no direct database,
     NATS, or arbitrary plugin network access from any field-device network (§17.2).
  4. Specify network resilience: what happens on link loss (edge buffering per §18.1),
     and the bounded offline-buffer size.
  5. Get the design reviewed by the customer's network/OT owner and record their approval
     or required changes.
- **Deliverable / output:** Site network design + zone/flow diagram per site, with
  allowlist and customer review record.
- **Acceptance criteria / exit check:** (a) every flow is documented with direction and
  initiator; (b) no flow is inbound from the internet into an OT network; (c) no
  undocumented flow exists between zones; (d) the customer network/OT owner has reviewed
  the design.
- **Owner role:** Platform engineering (with customer network/OT owner).
- **Dependencies:** P9.04. Feeds P9.06. Blocks P9.14.
- **Verification method:** Walk the diagram flow-by-flow against §17.2 and §3.3; confirm
  each flow has a matching firewall/allowlist rule in the customer environment (or a
  tracked action to create it).
- **Risks / failure modes:** A "temporary" inbound rule left in place; an undocumented
  vendor remote-access path; edge buffering unbounded so a long outage exhausts local
  storage (§18.3).

### P9.06 — OT/IT boundary review — D6 closure

- [ ] **P9.06 OT/IT boundary review — D6 closure**

- **Objective:** Close decision D6 for this deployment with a site-specific cyber and OT
  boundary review, before any read-only pilot begins.
- **Report trace:** §23 Stage 9; §17; §25.2 (D6); §22.3 ("before any read-only pilot").
- **Inputs / prerequisites:** P9.05 network design; P9.07 security assessment (draft);
  customer OT architecture, asset inventory, and existing OT security policies.
- **Steps:**
  1. Convene the review with the roles §17 requires: Neryva security, customer OT owner,
     customer IT owner, and the site operator.
  2. Walk every zone boundary (§17.2) and every flow in P9.05; record the decision for
     each: approved, approved with condition, or rejected with a required change.
  3. Assess the edge agent as an untrusted-by-default component (§18.2): it must
     authenticate each central session and validate every command; the central platform
     must not assume a command was applied without acknowledgement and telemetry evidence.
  4. Record the review outcome as the D6 closure record: scope, attendees, decisions,
     conditions, and residual risks carried to P9.27.
  5. If any boundary decision is "rejected", do not proceed to read-only pilot work at
     that site until it is re-reviewed and approved.
- **Deliverable / output:** D6 closure record for the deployment (signed by security owner
  and customer OT owner).
- **Acceptance criteria / exit check:** (a) every zone boundary has a recorded decision;
  (b) the review record is signed by the security owner and the customer OT owner;
  (c) no unresolved "rejected" boundary exists; (d) the record states the date and the
  deployment it covers — D6 closure is per deployment, not global.
- **Owner role:** Security (owner of D6 per `.agent/DECISIONS.md`).
- **Dependencies:** P9.05; P9.07 draft. Blocks P9.14, P9.17, and any read-only pilot
  activity at the site.
- **Verification method:** Review of the signed record against §17.2 and §18.2; confirm
  the attendee list includes the customer OT owner.
- **Risks / failure modes:** Review treated as a paperwork exercise without the customer
  OT owner present; a D6 closure recorded once and then reused for a different site or
  network (D6 is site-specific by definition, §25.2).

### P9.07 — Deployment security assessment

- [ ] **P9.07 Deployment security assessment**

- **Objective:** Assess the pilot deployment against the §17 threat model and required
  controls, using NIST SP 800-82 Rev. 3 as the final baseline and Rev. 4 only as a draft.
- **Report trace:** §23 Stage 9; §17.1–§17.4.
- **Inputs / prerequisites:** P9.01 scope; P9.04 integration design; P9.05 network design;
  the §17.3 control list and §17.4 threat table.
- **Steps:**
  1. For each §17.4 threat (forged/replayed command, compromised plugin, malicious or
     incorrect telemetry, cross-tenant leakage, OT lateral movement, broker
     duplication/replay, unauthorized operator action, plugin supply-chain compromise,
     clock drift, central outage), record the control that addresses it in this
     deployment and the evidence that the control is implemented — not merely planned.
     For each threat row, name the subtask that tests the control (P9.14/P9.15/P9.18/
     P9.19/P9.20/P9.23/P9.31/P9.32 as applicable); a control with no testing subtask
     is recorded as untested, not as implemented.
  2. Assess each §17.3 required control for the deployment (OIDC/OAuth2, RBAC plus
     scoped authorization, MFA/step-up where required, mTLS, short-lived credentials and
     rotation, signed commands with sequence/expiry/replay protection, plugin
     signature/provenance and SBOM checks, WASM sandbox limits, segmentation and
     allowlisting, encryption in transport and storage, secrets-manager integration,
     audit logging, edge secure boot/update where supported, backup/restore, vulnerability
     management, incident response).
  3. Use NIST SP 800-82 Rev. 3 (final, September 2023) as the baseline reference. Treat
     the Rev. 4 initial public draft (21 September 2026; comments due 30 November 2026)
     as draft input only: record any Rev. 4 change that would affect this deployment and
     mark it as draft, not as a requirement (see `RESEARCH.md`).
  4. Read IEEE 1547.3-2023 (guide for cybersecurity of DER interconnected with electric
     power systems) and record which of its recommendations apply to this deployment;
     do not assume its contents (see `RESEARCH.md` §2).
  5. Record every control that is missing, partial, or compensated, with the compensating
     measure and its owner.
  6. Define the pilot-period vulnerability-management cadence: scanning frequency, patch
     window, and owner for the deployed versions for the whole pilot period.
  7. Produce a findings list with severity, owner, and due state (must-fix before pilot
     vs accepted residual risk carried to P9.27).
- **Deliverable / output:** Deployment security assessment report + findings list.
- **Acceptance criteria / exit check:** (a) every §17.4 threat has a recorded control and
  evidence; (b) every §17.3 control has a status (implemented / partial / missing with
  compensation); (c) no must-fix finding is open at the P9.28 acceptance review; (d) Rev. 4
  is cited only as a draft, never as the baseline; (e) IEEE 1547.3 was read and its
  applicable recommendations are recorded; (f) the pilot-period scanning/patch cadence is
  recorded with an owner; (g) every §17.4 threat row names its testing subtask (or is
  recorded as untested).
- **Owner role:** Security.
- **Dependencies:** P9.04, P9.05. Feeds P9.06, P9.27. Blocks P9.28.
- **Verification method:** Independent review of the assessment by a second security
  reviewer; spot-check at least two controls per threat by inspecting configuration or
  test evidence, not by reading a claim.
- **Risks / failure modes:** Assessment based on the generic product design instead of the
  deployed configuration; Rev. 4 draft content treated as final requirements; a
  compensating control recorded without an owner or expiry.

### P9.08 — Identity, access, and approval design

- [ ] **P9.08 Identity, access, and approval design**

- **Objective:** Define who can do what at the pilot, under which identity, with which
  approval — so that operator-approved control (§21.4) is enforceable, not customary.
- **Report trace:** §23 Stage 9; §17.3; §20.2; §14 (authorization distinct from validation
  and feasibility).
- **Inputs / prerequisites:** P9.01 scope; customer identity provider details; P9.07
  assessment findings on identity controls.
- **Steps:**
  1. Define the pilot roles (e.g. pilot operator, approver, read-only observer, site
     technician, administrator) and the permissions of each, scoped by tenant/site/asset
     (§17.3, §5.7).
  2. Integrate the customer identity provider via OIDC/OAuth2 (§17.3); record the IdP,
     the claim mapping to roles, and the fallback if the IdP is unavailable.
  3. Require MFA and step-up approval for high-impact administrative/control actions
     where the customer requires it (§17.3); record which actions require step-up.
  4. Define separation of duties for dispatch approval: the person who proposes a command
     is not the person who approves it, unless the customer explicitly accepts a named
     exception in writing.
  5. Define the authorization evidence recorded per command (§14.3: authorization decision
     reference, approved scope, limits, expiry).
  6. Test the role matrix: each role attempts an action above its permission and is denied,
     with the denial audited (§17.4).
- **Deliverable / output:** Pilot role matrix + IdP integration record + step-up action list
  + negative-test evidence.
- **Acceptance criteria / exit check:** (a) every pilot user maps to exactly one role set
  via the IdP; (b) every high-impact action has a named step-up requirement or a written
  customer exception; (c) negative tests pass — denied actions are denied and audited;
  (d) no shared or generic accounts hold approval rights.
- **Owner role:** Security (with customer IT/IdP owner).
- **Dependencies:** P9.01, P9.07. Blocks P9.19, P9.21.
- **Verification method:** Execute the negative tests in the pilot environment and inspect
  the audit records; confirm no approval-capable account is shared.
- **Risks / failure modes:** Role mapped from an untrusted request field instead of
  authenticated claims (§16.2); a generic "pilot-admin" account used by several people,
  destroying audit attribution; step-up defined in a document but not enforced in the
  console (§20.2).

### P9.09 — Secrets, certificates, and key management

- [ ] **P9.09 Secrets, certificates, and key management**

- **Objective:** Ensure no secret, certificate, or key for the pilot lives in source control,
  a plugin config file, or an unencrypted store — and that rotation and expiry are handled
  before they bite.
- **Report trace:** §23 Stage 9; §17.3; §18.1; §18.3.
- **Inputs / prerequisites:** P9.04 integration design; P9.05 network design; customer
  secrets-manager / PKI capability.
- **Steps:**
  1. Inventory every secret and credential the pilot needs: edge mTLS identities, service
     identities, API keys, database credentials, signing keys, IdP client secrets.
  2. Assign each to a store: the customer secrets manager or the platform secrets
     integration (§17.3); record that no secret is stored in plugin config files or
     source control.
  3. Define certificate issuance, rotation, and expiry monitoring for edge and service
     identities (§17.3, §18.3: operator-visible health and certificate expiry).
  4. Define the edge key-storage posture: hardware-backed key storage where the customer
     platform supports it (§18.3); record where it is not available and the compensating
     control.
  5. Define revocation: how a compromised edge identity or operator credential is revoked
     and how quickly the revocation propagates.
  6. Test rotation on one non-production identity end-to-end before the pilot starts.
- **Deliverable / output:** Pilot secrets/certificate inventory + rotation and revocation
  procedure + rotation test record.
- **Acceptance criteria / exit check:** (a) every inventoried secret has a named store and
  no secret is found in source control or plugin config (verified by scan); (b) every
  certificate has a recorded expiry and a rotation owner; (c) the rotation test passed;
  (d) revocation procedure is written and its propagation time is measured, not assumed.
- **Owner role:** Security / platform engineering.
- **Dependencies:** P9.04, P9.05. Blocks P9.14.
- **Verification method:** Automated scan of the pilot repositories and config for secret
  patterns; inspect the rotation test log; confirm expiry monitoring is visible to the
  operator (§18.3).
- **Risks / failure modes:** A certificate expiring mid-pilot with no monitoring; an edge
  private key stored in plaintext on a gateway; revocation that only takes effect after a
  service restart nobody performs.

### P9.10 — Data residency, retention, and privacy design

- [ ] **P9.10 Data residency, retention, and privacy design**

- **Objective:** Fix, for this deployment, where data lives, how long each class is kept,
  and who may see it — driven by the jurisdiction (D1) and the customer contract, not by a
  universal default.
- **Report trace:** §23 Stage 9; §16.2–§16.3; §25.2 (D1).
- **Inputs / prerequisites:** P9.01 scope; D1 position; customer contract and applicable
  privacy/security requirements; market rules for the region (P9.11).
- **Steps:**
  1. Record the hosting mode for the pilot (§21.1: customer private cloud, customer
     on-premises, or hybrid) and the data-residency constraint it must satisfy.
  2. Define retention per data class (§16.3: raw high-frequency telemetry, normalized
     telemetry, aggregated intervals, command/audit evidence, market records, simulation
     artifacts, model/forecast outputs). Do not choose a universal retention period in
     code; record the per-class period and its basis (contract, market rule, or
     requirement).
  3. Confirm tenancy enforcement for the pilot: `tenant_id` on every tenant-owned record,
     authorization plus PostgreSQL RLS as defence in depth, application runtime role not
     owning tenant tables or bypassing RLS (§16.2).
  4. Record what personal/customer data the pilot holds, where it appears (including
     logs — §21.3 forbids secrets and unnecessary personal/customer data in logs), and
     how it is handled.
  5. Record the export/deletion procedure for the end of the pilot or at customer request.
- **Deliverable / output:** Pilot data-governance note: hosting mode, residency, per-class
  retention table, tenancy confirmation, personal-data handling, export/deletion procedure.
- **Acceptance criteria / exit check:** (a) every §16.3 data class has a recorded retention
  period with a stated basis; (b) tenancy enforcement is confirmed by test (cross-tenant
  access denied and audited); (c) no unnecessary personal/customer data is present in logs
  (verified by log inspection); (d) the export/deletion procedure is written.
- **Owner role:** Product + legal (retention/residency); platform engineering (tenancy test).
- **Dependencies:** P9.01; D1 position; P9.11 (market retention rules). Blocks P9.14.
- **Verification method:** Tenancy negative test in the pilot environment; retention
  configuration inspected against the recorded table; log sample inspected.
- **Risks / failure modes:** A market rule requiring longer retention than the contract
  allows, discovered after data was deleted; RLS relied on while the application runs as
  table owner (§16.2, [R25]); telemetry retained indefinitely because no class was ever
  assigned a period.

### P9.11 — Market / program requirements capture

- [ ] **P9.11 Market / program requirements capture**

- **Objective:** Capture the exact market or demand-response program rules the pilot must
  satisfy, as a region-specific adapter problem — not as a generic OpenADR mapping.
- **Report trace:** §23 Stage 9; §2.3; §15; §25.2 (D1); §11.4 (baseline rules are
  region/product-specific).
- **Inputs / prerequisites:** P9.01 scope; D1 position; the market operator / program
  documentation for the pilot region; P9.04 integration design.
- **Steps:**
  1. Name the market/program, the region, and the product (energy, capacity, ancillary
     service, demand response) — or record explicitly that the pilot has no market
     participation.
  2. Capture the program rules that drive FlexCore behaviour: event/program signal format,
     baseline and measurement method (§11.4), metering/telemetry requirements, delivery
     windows, settlement data, and reporting deadlines.
  3. For a US wholesale context, record the FERC Order 2222 implementation status for the
     relevant RTO/ISO (see `RESEARCH.md`: compliance filings approved for all six
     RTOs/ISOs, but live implementation staggered — CAISO live November 2024, ISO-NE
     November 2026, NYISO April 2027, PJM February 2028, MISO phased to June 2029, SPP
     Q2 2030). Do not claim universal market qualification or Order 2222 compliance
     (§0.3, §2.3).
  4. Map each program rule to the FlexCore adapter or procedure that implements it
     (§5.3 market/program adapter), and record rules FlexCore does not implement.
  5. Record the interconnection/agreement status for market participation (registration,
     aggregation approval, metering approval) or the plan to obtain it.
- **Deliverable / output:** Market/program requirements note + rule-to-adapter mapping +
  FERC 2222 status note for the region (or an explicit "no market participation" record).
- **Acceptance criteria / exit check:** (a) the market/program and product are named, or
  "no market participation" is recorded; (b) baseline/measurement rules are captured from
  the program documentation, not assumed; (c) no claim of universal market qualification
  or Order 2222 compliance appears anywhere in pilot material; (d) every program rule has
  an implementing adapter/procedure or a recorded gap.
- **Owner role:** Product (with legal for registration/agreement status).
- **Dependencies:** P9.01; D1 position. Feeds P9.12, P9.22. Blocks P9.28 where market
  participation is in scope.
- **Verification method:** Trace each captured rule to a page/section of the program
  documentation; confirm the FERC 2222 dates against the sources in `RESEARCH.md`.
- **Risks / failure modes:** Designing to a neighbouring region's program rules; assuming
  Order 2222 implementation in an RTO/ISO where it is not yet live; baseline method
  chosen by FlexCore instead of by the program.

### P9.12 — Interconnection and authority engagement

- [ ] **P9.12 Interconnection and authority engagement**

- **Objective:** Identify every external approval the pilot needs (utility/DSO, market
  operator, interconnection authority), engage them, and track each approval to a decision.
- **Report trace:** §23 Stage 9; §22.3 ("any applicable market/interconnection
  qualification"); §2.3.
- **Inputs / prerequisites:** P9.01 scope; P9.03 network model; P9.11 market/program note;
  the applicable interconnection requirements (e.g. IEEE 1547-2018 as adopted in the
  jurisdiction — see `RESEARCH.md`).
- **Steps:**
  1. List the authorities for the pilot: distribution utility/DSO, interconnection
     authority, market operator (if any), and any other applicable authority.
  2. For each, record what approval is needed, the submission made (or to be made), the
     reference number, and the decision or expected decision date.
  3. Record the interconnection standard and profile that applies at each site (e.g.
     IEEE 1547-2018 category and settings, as adopted locally — the standard's Clause 5
     capabilities depend on category, utility profile, and equipment, §11.3).
  4. Record any study the authority requires (e.g. distribution impact study, cf. IEEE
     1547.7) and who performs it.
  5. Track each item to closure or to an explicit "not required, with basis" record.
- **Deliverable / output:** Authority-engagement tracker: authority, approval needed,
  submission reference, status, decision.
- **Acceptance criteria / exit check:** (a) every applicable authority is listed; (b) every
  item has a status of approved, submitted with reference, or not-required with a stated
  basis — no item is unlisted; (c) the interconnection standard/profile per site is
  recorded.
- **Owner role:** Product (with power systems for technical submissions).
- **Dependencies:** P9.03, P9.11. Feeds P9.29. Blocks P9.28 where an approval is a
  precondition.
- **Verification method:** Inspect the tracker against the authority correspondence;
  confirm each "not required" has a cited basis.
- **Risks / failure modes:** An authority engaged late, after the pilot schedule is fixed;
  assuming a neighbouring jurisdiction's adoption of IEEE 1547-2018 applies locally;
  a required impact study discovered during field acceptance.

### P9.13 — Site commissioning plan

- [ ] **P9.13 Site commissioning plan**

- **Objective:** Write the step-by-step plan that takes each site from "installed" to
  "accepted for shadow operation", with a named owner and pass/fail per step.
- **Report trace:** §23 Stage 9; §19.2 (HIL/commissioning tests); §21.2.
- **Inputs / prerequisites:** P9.02–P9.05, P9.09; vendor commissioning documentation; site
  access and safety procedures from the customer.
- **Steps:**
  1. Write the commissioning sequence per site: physical install check, power and network
     check, edge-agent enrolment, identity/certificate check, telemetry flow check,
     command-path check (disabled), fallback check.
  2. For each step, record the procedure, the expected result, the pass/fail criterion,
     the owner, and the evidence to file (log, screenshot, signed checklist).
  3. Define the site safety rules for commissioning: who may touch equipment, lock-out /
     tag-out or equivalent where the site requires it, and the stop condition for any step.
  4. Define the rollback for a failed step: what is restored, and who decides whether to
     retry, defer, or abort the site.
  5. Include the read-only/shadow entry condition: the site enters shadow operation only
     when every commissioning step has passed and D6 is closed for the site (P9.06).
- **Deliverable / output:** Site commissioning plan per site (steps, criteria, owners,
  evidence, rollback, safety rules).
- **Acceptance criteria / exit check:** (a) every step has a pass/fail criterion and a
  named evidence artifact; (b) safety rules and stop conditions are written; (c) the
  shadow-entry condition explicitly requires D6 closure; (d) the customer site contact has
  reviewed the plan.
- **Owner role:** Integration engineering (with customer site contact).
- **Dependencies:** P9.02–P9.05, P9.09. Blocks P9.14, P9.17.
- **Verification method:** Walkthrough of the plan with the customer site contact against
  the vendor commissioning documentation; confirm no step lacks a criterion.
- **Risks / failure modes:** Commissioning steps written without pass/fail criteria (so
  everything "passes"); a site entering shadow operation before D6 closure; safety rules
  assumed from another site.

### P9.14 — Lab integration test — read-only end-to-end

- [ ] **P9.14 Lab integration test — read-only end-to-end**

- **Objective:** Re-verify the Stage 6 exit gate for the pilot configuration in the lab:
  read-only end-to-end site integration works, and the command path stays disabled unless
  explicitly approved for a test environment.
- **Report trace:** §23 Stage 9; §19.2; §23 Stage 6 exit gate.
- **Inputs / prerequisites:** P9.02–P9.06, P9.09, P9.10, P9.13; lab environment matching
  the pilot adapter/protocol versions.
- **Steps:**
  1. Stand up the pilot configuration in the lab: control plane, edge agent, site EMS /
     protocol adapters, and the pilot grid model (P9.03).
  2. Verify telemetry end-to-end: device → adapter → edge → control plane → operator
     console, with quality states (§10.4) and units/sign (§10.2) checked at each boundary.
  3. Verify the command path is disabled by default: attempt a command and confirm it is
     rejected before issue, with the rejection audited.
  4. Run the §19.2 fault-injection subset applicable to the lab: dropped, reordered, and
     duplicated messages; stale telemetry; unavailable asset; solver timeout.
  5. Record every result with the software versions, model version, and seed/config used.
- **Deliverable / output:** Lab integration test report (configuration, results per case,
  versions, faults injected, outcomes).
- **Acceptance criteria / exit check:** (a) telemetry is correct end-to-end with quality
  and units verified; (b) the command path is demonstrably disabled in the lab default
  configuration; (c) every injected fault has a recorded outcome and no fault produces a
  silent wrong value; (d) the report records versions and seeds so the run is repeatable.
- **Owner role:** Integration engineering (with platform engineering).
- **Dependencies:** P9.02–P9.06, P9.09, P9.10, P9.13; D6 closed. Blocks P9.15, P9.17.
- **Verification method:** Re-run a sample of the test cases from the recorded
  configuration and compare outcomes; inspect the command-rejection audit record.
- **Risks / failure modes:** Lab adapter version differs from the field version; fault
  injection skipped as "covered by unit tests"; a command accidentally enabled in the lab
  and treated as evidence the field path works.

### P9.15 — Hardware-in-the-loop test plan and execution

- [ ] **P9.15 Hardware-in-the-loop test plan and execution**

- **Objective:** Prove, with real controller/equipment hardware in the loop, that the
  command/control integration behaves correctly — before any field command is issued.
- **Report trace:** §23 Stage 9; §19.1 (HIL track); §19.2 item 10 (HIL only after
  simulation, integration, and cyber reviews pass).
- **Inputs / prerequisites:** P9.14 lab results; P9.06 D6 closure; P9.07 assessment with no
  open must-fix findings; HIL platform access (vendor/partner lab); the pilot grid model
  (P9.03) and adapter set (P9.04).
- **Steps:**
  1. Define the HIL scope: controller HIL (CHIL — real controller against a simulated
     grid) and/or power HIL (PHIL — real inverter/equipment against a simulated grid via
     a power amplifier), per what the pilot equipment and the lab support. Record which
     applies and why; do not claim PHIL evidence from a CHIL-only run.
  2. Select the HIL platform (e.g. OPAL-RT, RTDS, Typhoon HIL — see `RESEARCH.md`) and
     record its capabilities, time step, and interface method (for PHIL, the interface
     algorithm and its stability condition). Record the platform's calibration/
     validation evidence (last calibration/validation date and record); a run on an
     unvalidated rig is not HIL evidence.
  3. Define the test cases from IEEE 1547.1-2020 conformance test procedures where they
     apply (volt-var, volt-watt, constant power factor, ride-through, frequency
     response), plus FlexCore-specific cases: approved command delivery, acknowledgement,
     expiry, supersede, fallback activation, and communication loss.
  4. Run the cases in the order §19.2 requires: only after simulation, integration, and
     cyber reviews have passed — record the gate evidence for each.
  5. Record per case: setup, inputs, expected behaviour, observed behaviour, pass/fail,
     and the equipment/firmware versions under test.
  6. File failures with a disposition (fixed and re-run / accepted limitation in P9.27 /
     blocks the pilot).
- **Deliverable / output:** HIL test plan + execution report with per-case results and
  equipment/firmware versions.
- **Acceptance criteria / exit check:** (a) the HIL scope (CHIL vs PHIL) is stated and
  matches the evidence; (b) every defined case has a recorded pass/fail; (c) no case is
  marked passed on a different equipment/firmware version than the pilot's; (d) every
  failure has a disposition; (e) the pre-gate evidence (simulation, integration, cyber
  reviews passed) is filed.
- **Owner role:** Power systems engineering (with the HIL lab / vendor partner).
- **Dependencies:** P9.14; P9.06; P9.07 (no open must-fix). Blocks P9.17.
- **Verification method:** Independent review of the HIL report by a power-systems
  reviewer who did not run the tests; spot-check at least one case by re-running it or
  inspecting the raw capture.
- **Risks / failure modes:** HIL run before the cyber review passes (§19.2 forbids this);
  PHIL interface instability mistaken for equipment failure; a test passed on lab firmware
  that differs from field firmware; HIL evidence used to imply equipment certification —
  it does not (conformance testing under IEEE 1547.1 remains a separate responsibility,
  §11.3).

### P9.16 — Field acceptance test plan

- [ ] **P9.16 Field acceptance test plan**

- **Objective:** Write the field acceptance tests (FAT/SAT) that the customer and the
  applicable authorities will use to judge the pilot — before the tests are run.
- **Report trace:** §23 Stage 9; §19.2; §22.3.
- **Inputs / prerequisites:** P9.01 scope; P9.13 commissioning plan; P9.15 HIL results;
  P9.11 market/program requirements; customer acceptance criteria.
- **Steps:**
  1. Define the acceptance test cases per site: telemetry accuracy and freshness,
     flexibility-envelope sanity, schedule validation, command lifecycle in shadow,
     acknowledgement and tracking, fallback behaviour, operator workflow (§20), audit
     completeness.
  2. For each case, record the procedure, the input/starting state, the expected result,
     the pass/fail threshold (set before the run), and the evidence to file.
  3. Define the acceptance-test environment state: control-authority mode (§21.4),
     software/model versions frozen for the run, and the rule that a version change during
     the run invalidates the affected cases.
  4. Define the roles present during acceptance (customer operator, Neryva engineer, site
     contact, authority witness where required) and who signs each case.
  5. Define the failure process: a failed case is retried only after a recorded fix or a
     recorded waiver signed by the customer — never by re-running until it passes.
  6. Define the re-run set: which cases must re-run after a version change during the run
     (affected-subset rule, stated in advance) and which changes force a full re-run.
- **Deliverable / output:** Field acceptance test plan per site (cases, thresholds, roles,
  sign-off, failure process).
- **Acceptance criteria / exit check:** (a) every case has a pre-set threshold; (b) the
  frozen versions for the run are recorded; (c) the failure process forbids silent
  re-runs; (d) the customer has reviewed and agreed the plan; (e) the re-run set for a
  mid-run version change is defined in advance.
- **Owner role:** Integration engineering (with product and the customer).
- **Dependencies:** P9.01, P9.13, P9.15. Blocks P9.17.
- **Verification method:** Review the plan against §19.2 and §19.4 metrics; confirm no
  threshold was set after a result was known.
- **Risks / failure modes:** Thresholds fitted to observed results; a version change
  mid-run silently invalidating earlier cases; acceptance witnessed only by Neryva.

### P9.17 — Field acceptance test execution — shadow first

- [ ] **P9.17 Field acceptance test execution — shadow first**

- **Objective:** Execute the field acceptance tests at the real sites, in read-only/shadow
  mode first, and only then — with approval — in operator-approved mode.
- **Report trace:** §23 Stage 9; §19.2; §21.4 (mode promotion requires §22.3 gates plus
  site-specific acceptance).
- **Inputs / prerequisites:** P9.13 commissioning complete per site; P9.14, P9.15, P9.16;
  P9.06 D6 closed; frozen versions per P9.16.
- **Steps:**
  1. Confirm each site has completed commissioning (P9.13) and is in read-only/shadow mode.
  2. Execute the P9.16 cases in shadow mode: FlexCore computes and compares proposed
     actions against measurements without issuing commands (§21.4).
  3. Record every result with the frozen versions, timestamps, and the operator present.
     File failures per the P9.16 failure process.
  4. Hold a shadow-review: the customer reviews shadow results (including tracking error
     and any rejected commands with their reasons) before any mode promotion is requested.
  5. Only after shadow acceptance, request promotion to operator-approved mode per P9.26,
     and execute the operator-approved acceptance cases (approved command issue,
     acknowledgement, tracking, completion) under direct operator supervision.
  6. If any operator-approved case fails, revert the site to shadow mode and record the
     reversion; do not continue issuing commands at that site.
- **Deliverable / output:** Field acceptance test execution record per site (shadow results,
  shadow-review record, promotion request, operator-approved results, any reversions).
- **Acceptance criteria / exit check:** (a) all shadow cases pass or have signed waivers;
  (b) the shadow review happened before any promotion request; (c) operator-approved cases
  were executed only after shadow acceptance; (d) any failed operator-approved case
  resulted in a recorded reversion to shadow.
- **Owner role:** Integration engineering (with the customer operator).
- **Dependencies:** P9.13–P9.16; P9.06. Blocks P9.26, P9.28.
- **Verification method:** Inspect the execution record for version consistency and for
  the shadow-before-promotion order; confirm the customer operator's signature on the
  shadow review.
- **Risks / failure modes:** Skipping shadow mode to save time; a failed command treated
  as a "test artifact" and the site left in operator-approved mode; acceptance evidence
  recorded without the versions that produced it.

### P9.18 — Site telemetry quality and clock-sync verification

- [ ] **P9.18 Site telemetry quality and clock-sync verification**

- **Objective:** Prove the telemetry the pilot will decide on is trustworthy at each site:
  correctly timestamped, correctly quality-flagged, and fresh enough for its use case.
- **Report trace:** §23 Stage 9; §10.3–§10.4; §17.4 (clock drift, malicious/incorrect
  telemetry).
- **Inputs / prerequisites:** P9.14 lab results; P9.17 shadow operation; site telemetry
  sources and their synchronization method.
- **Steps:**
  1. For each telemetry source, verify the time model (§10.3): `event_time` vs
     `ingest_time` distinguished, UTC internally, site timezone retained for display,
     source sequence/version recorded.
  2. Verify clock synchronization at the site and the edge: record the sync source and the
     measured offset/drift; verify command expiry still protects against the observed
     drift (§17.4).
  3. Verify quality mapping (§10.4): each source's native states map to exactly one
     canonical state (`GOOD`, `SUSPECT`, `BAD`, `MISSING`, `ESTIMATED`, `STALE`), with
     the original source code retained as diagnostic metadata; no quality flag is silently
     converted into a normal reading.
  4. Measure freshness per use case: ingestion delay and staleness against the freshness
     limit for each use case; record which quality states each use case accepts.
  5. Verify stale/missing handling end-to-end: inject a stale feed and a missing feed and
     confirm the system holds/rejects per policy instead of inferring values (§17.5
     invariant 9: a missing/uncertain/stale model is not a passing feasibility check).
- **Deliverable / output:** Telemetry verification report per site (sync measurements,
  quality mapping table, freshness measurements, stale/missing injection results).
- **Acceptance criteria / exit check:** (a) every source has a recorded quality mapping to
  exactly one canonical state; (b) measured clock offset is within the bound the command
  expiry assumes; (c) freshness meets the per-use-case limit; (d) stale/missing injection
  produces hold/reject, never a silently substituted value.
- **Owner role:** Platform engineering (with integration engineering).
- **Dependencies:** P9.14, P9.17 (shadow data). Blocks P9.26.
- **Verification method:** Re-inspect the mapping table against adapter code/config;
  re-run one stale-injection case and confirm the outcome.
- **Risks / failure modes:** A source clock drifting beyond the expiry bound, making
  "fresh" commands stale; `ESTIMATED` values treated as measured in a schedule; a quality
  mapping that collapses `SUSPECT` into `GOOD`.

### P9.19 — Command-lifecycle dry run in shadow mode

- [ ] **P9.19 Command-lifecycle dry run in shadow mode**

- **Objective:** Exercise the full command state machine (§14.1) against real site
  conditions without issuing a physical command, and prove each gate is distinct.
- **Report trace:** §23 Stage 9; §14; §21.4 (read-only/shadow).
- **Inputs / prerequisites:** P9.03 grid model; P9.14 lab results; P9.17 shadow operation;
  P9.08 role matrix.
- **Steps:**
  1. Generate candidate commands from the pilot schedules and run them through the full
     chain: PROPOSED → VALIDATED → FEASIBLE → APPROVED (simulated approval by the
     pilot approver role) → held before ISSUED, with the hold recorded.
  2. Verify each gate separately: a malformed candidate fails validation; an infeasible
     candidate fails feasibility; an unauthorized candidate fails authorization — and a
     candidate that fails one gate is not passed by another (§14.1: validation, feasibility,
     and authorization are distinct).
  3. Exercise the terminal and failure transitions in shadow: REJECTED, EXPIRED (validity
     window elapsed), SUPERSEDED (newer approved revision replaces an older one),
     CANCELLED.
  4. Verify command invariants (§14.3) on every dry-run command: unique ID, tenant/
     portfolio/site/asset IDs, source and schedule revision, validity window, sequence,
     idempotency key, model/telemetry snapshot references, validation and authorization
     evidence references.
  5. Verify the audit trail records each transition with actor, decision, and evidence
     (§10.1 AuditRecord).
- **Deliverable / output:** Shadow command-lifecycle dry-run report (cases per transition,
  gate-separation evidence, invariant checks, audit extracts).
- **Acceptance criteria / exit check:** (a) every §14.1 transition exercised in shadow has
  a recorded outcome; (b) at least one candidate fails each of validation, feasibility,
  and authorization independently; (c) no command reaches ISSUED during the dry run;
  (d) every dry-run command carries the full §14.3 invariant set.
- **Owner role:** Platform engineering (with the pilot approver).
- **Dependencies:** P9.03, P9.08, P9.14, P9.17. Blocks P9.26.
- **Verification method:** Inspect the audit trail for the dry-run commands; confirm no
  ISSUED state exists for any dry-run command ID.
- **Risks / failure modes:** A gate that passes everything in shadow because the validator
  is pointed at a stale model; approval simulated by the same person who proposed the
  command (violating P9.08 separation of duties); an expired command treated as issuable.

### P9.20 — Fallback and degraded-mode drills

- [ ] **P9.20 Fallback and degraded-mode drills**

- **Objective:** Prove each site falls back safely when FlexCore, the network, or an asset
  fails — with the fallback defined per asset/site, not as a universal "set power to zero".
- **Report trace:** §23 Stage 9; §14.4; §17.4 (central outage); §21.2 (degraded modes).
- **Inputs / prerequisites:** P9.02 asset register; P9.04 integration design; P9.13
  commissioning plan; the per-asset/site fallback policies.
- **Steps:**
  1. Record the fallback policy per asset/site (§14.4): hold last accepted setpoint for a
     bounded interval where approved, return to local EMS autonomous control, preserve
     reserve/SOC constraints, stop accepting new external commands, request a safe local
     mode, or require operator intervention. A last-known setpoint is not automatically
     safe — record the review with the equipment/site operator for each policy.
  2. Drill each degraded mode: central outage (site-local fallback and autonomous local
     EMS, §17.4), edge-to-central link loss (offline buffering, §18.1), asset
     unavailability, solver timeout, stale telemetry (§19.2 fault injection),
     plugin-host crash, gRPC worker crash.
  3. For each drill, record the trigger, the observed fallback, the time to fallback, and
     whether site-local safety/protection remained independent of FlexCore (§17.5
     invariant 10).
  4. Verify that a central outage does not disable site-local protective controls or force
     an unsafe device state (§21.2).
  5. File any drill failure with a disposition; re-drill after a fix.
- **Deliverable / output:** Fallback policy register per asset/site + drill report (mode,
  trigger, observed behaviour, time to fallback, disposition of failures).
- **Acceptance criteria / exit check:** (a) every pilot asset has a recorded fallback policy
  reviewed with the equipment/site operator; (b) every listed degraded mode has a drill
  with a recorded outcome; (c) no drill shows a central outage disabling local protection
  or forcing an unsafe state; (d) no drill failure is open at P9.28.
- **Owner role:** Integration engineering (with power systems and the site operator).
- **Dependencies:** P9.02, P9.04, P9.13. Blocks P9.26, P9.28.
- **Verification method:** Witness at least one drill (central-outage drill) in person or
  by recorded evidence; inspect the fallback register for operator review signatures.
- **Risks / failure modes:** A universal zero-setpoint fallback applied to an asset for
  which it is unsafe; a fallback that holds a stale setpoint after topology changed
  (§14.4); a drill that "passes" because the failure was never actually injected.

### P9.21 — Operator training and role qualification

- [ ] **P9.21 Operator training and role qualification**

- **Objective:** Qualify the named people who will operate and approve at the pilot, on the
  actual pilot system — not on a generic demo.
- **Report trace:** §23 Stage 9; §20 (operator application design); §17.3.
- **Inputs / prerequisites:** P9.08 role matrix; P9.17 shadow operation; the pilot operator
  console (§20.1 workspaces).
- **Steps:**
  1. Write the training plan per role: console workspaces (§20.1), reading telemetry
     quality and freshness, distinguishing proposed/feasible/approved/issued/
     acknowledged/measured states (§20.2), approving and rejecting commands, handling
     alarms, and the emergency/rollback actions.
  2. Train each named operator and approver on the pilot system with pilot data; record
     attendance and the material version used.
  3. Assess each operator with a practical exercise: approve a valid command, reject an
     infeasible one with its stated reason, recognise a stale-telemetry state, and execute
     the fallback/rollback procedure.
  4. Record qualification per person and role; an unqualified person holds no approval
     rights in the pilot configuration (P9.08 enforcement).
  5. Provide the operator reference: the market/product-specific procedures (P9.22) and
     the incident procedure (P9.23) in the form the operator will actually use.
- **Deliverable / output:** Training plan + attendance/assessment records + per-person
  qualification register.
- **Acceptance criteria / exit check:** (a) every person with approval rights has passed
  the practical exercise; (b) no approval right is held by an unqualified person in the
  pilot configuration; (c) the emergency/rollback procedure was exercised, not just read.
- **Owner role:** Product / operations (with the customer operator lead).
- **Dependencies:** P9.08, P9.17. Blocks P9.26.
- **Verification method:** Inspect the qualification register against the P9.08 role
  matrix and the live configuration; confirm the practical exercise records exist per
  approver.
- **Risks / failure modes:** Training on a demo system that differs from the pilot build;
  an operator who can approve but cannot recognise a stale or estimated value (§20.2);
  qualification recorded for a role the person does not hold.

### P9.22 — Market / product-specific operational procedures

- [ ] **P9.22 Market / product-specific operational procedures**

- **Objective:** Write the day-to-day operating procedures for the pilot's specific service
  and market/program — the procedures the operator follows when an event arrives.
- **Report trace:** §23 Stage 9; §1.5; §15; §11.4.
- **Inputs / prerequisites:** P9.11 market/program requirements; P9.01 service scope;
  P9.17 shadow experience.
- **Steps:**
  1. Write one procedure per operational situation in scope: day-ahead/intraday scheduling
     cycle, demand-response event receipt and response, peak-management action, market
     commitment tracking, settlement-data preparation, and end-of-day reconciliation.
  2. Each procedure states: trigger, inputs (including which telemetry quality states are
     acceptable), steps, the approval point, the expected outcome, and the failure path
     (including what happens if the event signal is late, duplicated, or malformed).
  3. Record the baseline/measurement method for the program (§11.4) as the program defines
     it, and the evidence retained for settlement.
  4. Walk each procedure with the customer operator against a real shadow case (P9.17) and
     correct it where the walkthrough diverges from practice.
  5. Version the procedures and record the version the pilot operates under.
- **Deliverable / output:** Pilot operations manual (procedures, version, walkthrough
  corrections).
- **Acceptance criteria / exit check:** (a) every in-scope §1.5 use case has a procedure;
  (b) every procedure names its trigger, approval point, and failure path; (c) baseline/
  measurement follows the program's definition; (d) each procedure was walked through
  against a shadow case.
- **Owner role:** Product / operations (with the customer operator).
- **Dependencies:** P9.11, P9.17. Blocks P9.28.
- **Verification method:** Walkthrough review: pick one procedure and trace it against a
  shadow case record; confirm the failure path is written, not implied.
- **Risks / failure modes:** A procedure written from the generic product manual instead
  of the program's rules; an event signal with no defined handling for duplicates (§15.4
  at-least-once delivery); settlement evidence retained in a form the program does not
  accept.

### P9.23 — Incident response and customer notification procedure

- [ ] **P9.23 Incident response and customer notification procedure**

- **Objective:** Define, for this pilot, what counts as an incident, who is called, in what
  order, and how the customer is notified — before an incident happens.
- **Report trace:** §23 Stage 9; §17.3 (incident response and customer notification process);
  §17.4.
- **Inputs / prerequisites:** P9.07 security assessment; P9.01 scope; customer escalation
  contacts and any contractual notification deadlines.
- **Steps:**
  1. Define incident classes for the pilot (e.g. suspected command forgery/replay,
     unauthorized operator action, cross-tenant access attempt, edge compromise, telemetry
     integrity failure, central outage during an active dispatch).
  2. For each class, define: detection source, first responder, escalation path, the
     customer contact and notification deadline, and the immediate containment action
     (including reverting sites to shadow or local control).
  3. Define the evidence preserved per incident (audit records, telemetry snapshots,
     command records) and the rule that evidence is preserved before any remediation
     deletes it.
  4. Run a tabletop exercise with the customer for at least one incident class (a forged/
     replayed command attempt) and record the outcome and corrections.
  5. Record the on-call rota and contact details for the pilot period, and the procedure
     for updating them.
- **Deliverable / output:** Pilot incident-response and notification procedure + tabletop
  exercise record + on-call rota.
- **Acceptance criteria / exit check:** (a) every incident class has a detection source,
  escalation path, customer contact, and notification deadline; (b) containment includes
  an explicit revert-to-shadow/local-control action; (c) the tabletop exercise was held
  with the customer and its corrections applied; (d) the on-call rota is current.
- **Owner role:** Security (with product/operations and the customer).
- **Dependencies:** P9.07. Blocks P9.28.
- **Verification method:** Inspect the tabletop record for customer attendance and for a
  containment decision actually exercised; verify the rota contacts by a test call or
  message where the customer permits it.
- **Risks / failure modes:** A notification deadline in the contract that the procedure
  does not meet; evidence destroyed by a well-meaning remediation; an incident class with
  no detection source (so it would only be found by the customer).

### P9.24 — Backup, restore, and disaster-recovery rehearsal

- [ ] **P9.24 Backup, restore, and disaster-recovery rehearsal**

- **Objective:** Prove the pilot can be restored from backup — by actually restoring it —
  and that the edge survives a central outage within its bounded buffer.
- **Report trace:** §23 Stage 9; §17.3 (backup/restore tests); §21.2.
- **Inputs / prerequisites:** P9.10 retention design; the pilot deployment (control plane,
  PostgreSQL, NATS JetStream, edge agents); the backup configuration.
- **Steps:**
  1. Record the backup scope: PostgreSQL (transactional source of truth, §16.1), NATS
     stream replication/retention design, durable command/audit records, edge offline
     buffering (§21.2).
  2. Perform a restore rehearsal in an isolated environment: restore PostgreSQL from
     backup, verify command/audit records and the outbox state, and verify the restored
     system reaches a consistent state (idempotent replay, §6.3/§15.5).
  3. Measure and record the restore time and the data-loss window actually observed; do
     not record RPO/RTO values that were never measured (§21.2: do not set RPO/RTO
     without customer and service requirements — record the measured values and the
     requirements they are judged against).
  4. Test edge behaviour during a central outage: confirm bounded offline buffering,
     store-and-forward on recovery, and that no command is issued from a stale buffered
     state.
  5. Record the disaster-recovery procedure that the rehearsal validated, with its
     measured timings.
- **Deliverable / output:** Backup/restore rehearsal report (scope, restore steps, measured
  time and loss window, edge-outage test, validated DR procedure).
- **Acceptance criteria / exit check:** (a) a restore was actually performed and verified,
  not just a backup job observed; (b) restored command/audit records are complete and
  consistent; (c) measured restore time and loss window are recorded against the customer
  requirements; (d) the edge outage test shows bounded buffering and no stale command
  issue.
- **Owner role:** Platform engineering.
- **Dependencies:** P9.10. Blocks P9.28; feeds P10.11 (Phase 10 re-runs this against the
  release candidate — cross-reference, do not duplicate the procedure here).
- **Verification method:** Inspect the rehearsal log and the restored database's audit
  record count against the source; confirm the edge test evidence.
- **Risks / failure modes:** A backup that has never been restored (the classic failure);
  an outbox state that replays already-issued commands after restore (§15.5 idempotency
  must hold); a restore that succeeds but with an older schema than the running code.

### P9.25 — Observability and alerting verification at the pilot

- [ ] **P9.25 Observability and alerting verification at the pilot**

- **Objective:** Prove the pilot's monitoring actually detects the failures that matter, with
  alerts that reach a named person — not just dashboards that exist.
- **Report trace:** §23 Stage 9; §21.3.
- **Inputs / prerequisites:** P9.17 shadow operation; P9.20 drill results; the pilot
  deployment's observability configuration.
- **Steps:**
  1. Verify correlation on pilot records (§21.3): tenant/site/portfolio/asset IDs, schedule
     and command IDs, plugin/solver/model versions, telemetry snapshot and grid-model
     version, trace ID, actor identity, validation/authorization evidence.
  2. Verify the measured signals (§21.3): ingestion delay, stale telemetry, queue depth,
     plugin health, solver duration/status, command delivery, acknowledgement, and tracking
     deviation — each with a defined alert threshold and a named recipient.
  3. Trigger at least one alert per critical class (stale telemetry, command acknowledgement
     timeout, solver failure) in a controlled way and confirm it reaches the named recipient
     within the expected time.
  4. Verify logs contain no secrets and no unnecessary personal/customer data (§21.3) by
     inspecting a sample of pilot logs.
  5. Record the alert catalogue: signal, threshold, recipient, escalation, and the test
     evidence per alert.
- **Deliverable / output:** Pilot observability verification report + alert catalogue with
  test evidence.
- **Acceptance criteria / exit check:** (a) every §21.3 correlation field is present on a
  sampled pilot record; (b) every critical alert class was triggered and received;
  (c) the log sample contains no secrets or unnecessary personal/customer data;
  (d) every alert has a named recipient and an escalation.
- **Owner role:** Platform engineering.
- **Dependencies:** P9.17, P9.20. Blocks P9.28.
- **Verification method:** Trigger-and-receive test for one alert class witnessed by the
  recipient; inspect a sampled trace end-to-end for the correlation fields.
- **Risks / failure modes:** Alerts routed to a mailbox nobody reads; a threshold set so
  loose the failure never fires; correlation IDs missing across the edge/central boundary,
  making a pilot incident untraceable.

### P9.26 — Control-scope document and operator-approved configuration

- [ ] **P9.26 Control-scope document and operator-approved configuration**

- **Objective:** Write down the exact control scope the pilot is approved for, and configure
  the deployed system so it cannot exceed that scope.
- **Report trace:** §23 Stage 9; §21.4; §22.3 (target control-authority mode qualification).
- **Inputs / prerequisites:** P9.17 shadow acceptance; P9.18–P9.21; P9.01 scope.
- **Steps:**
  1. Write the control-scope document: per site and asset class, the authorised mode
     (operator-approved), the asset set, the power/energy limits, the validity windows,
     the approved adapters, and the expiry/review date of the scope.
  2. Configure the deployment to enforce it: mode is a deployment/per-site setting (§21.4);
     verify the running configuration matches the document (mode, site list, asset list,
     limits) by inspecting the deployed configuration, not the design document.
  3. Verify that no plugin can select or raise the mode (§21.4; §5.7: `dispatch.authorize`
     never granted to general plugins).
  4. Verify the promotion path: a site moves from shadow to operator-approved only through
     the P9.17 shadow-review and this document — record the promotion decision per site.
  5. Record the explicit exclusion: bounded automation is not enabled anywhere in the
     pilot, and the configuration contains no automation envelope.
- **Deliverable / output:** Control-scope document (per site/asset class) + deployed-
  configuration verification record + per-site promotion decisions.
- **Acceptance criteria / exit check:** (a) the deployed configuration matches the document
  field-for-field for mode, sites, assets, and limits; (b) no plugin holds
  `dispatch.authorize` or can change the mode; (c) every site in operator-approved mode
  has a recorded promotion decision after shadow acceptance; (d) no automation envelope
  exists in the configuration.
- **Owner role:** Product (scope) + platform engineering (configuration verification).
- **Dependencies:** P9.17, P9.18, P9.19, P9.20, P9.21. Blocks P9.28.
- **Verification method:** Independent inspection of the running configuration against the
  document; attempt a mode change via a plugin path and confirm it is refused.
- **Risks / failure modes:** Configuration drift between the document and the deployment;
  a site promoted on a verbal approval; an automation envelope left enabled from a lab
  profile (e.g. a research profile, §5.8) in the pilot deployment.

### P9.27 — Known-limitations and residual-risk register

- [ ] **P9.27 Known-limitations and residual-risk register**

- **Objective:** Write down, honestly, what the pilot does not do, does not cover, and still
  risks — so the acceptance decision in P9.28 is informed, not surprised.
- **Report trace:** §23 Stage 9; §17.4; §22.3; §0.3 (deliberately not promised).
- **Inputs / prerequisites:** All prior Phase 9 subtasks; P9.07 findings; P9.03 model
  coverage limitations; P9.15 HIL dispositions.
- **Steps:**
  1. List every known limitation: model coverage gaps (P9.03), adapter gaps (P9.04),
     market rules not implemented (P9.11), HIL limitations (P9.15), open D1–D3/D5
     positions, and any §0.3 item relevant to the pilot.
  2. List every residual risk: threat controls that are partial or compensated (P9.07),
     accepted findings, and the operational risks from the drills (P9.20).
  3. For each item, record: description, impact if it materialises, likelihood as assessed
     (with the basis for the assessment), owner, mitigation or monitoring, and expiry/
     review date. Do not record a likelihood without a basis.
  4. Have the customer review the register and acknowledge the items that affect their
     sites.
  5. Carry the register forward as an input to Phase 10 (P10.26 release notes) — the
     release notes must not omit a limitation recorded here.
- **Deliverable / output:** Known-limitations and residual-risk register, customer-
  acknowledged.
- **Acceptance criteria / exit check:** (a) every limitation from P9.03, P9.04, P9.11, and
  P9.15 appears in the register or is explicitly closed with evidence; (b) every residual
  risk has an owner and a review date; (c) the customer has acknowledged the register;
  (d) no item is worded as a certification or safety claim.
- **Owner role:** Product (with security and power systems).
- **Dependencies:** P9.03, P9.04, P9.07, P9.11, P9.15, P9.20. Blocks P9.28.
- **Verification method:** Cross-check the register against the source subtask outputs
  item by item; confirm the customer's acknowledgement is on file.
- **Risks / failure modes:** Limitations softened into marketing language; a risk recorded
  without an owner, so nobody watches it; a limitation omitted from the release notes in
  Phase 10.

### P9.28 — Customer and site acceptance review

- [ ] **P9.28 Customer and site acceptance review**

- **Objective:** Hold the formal acceptance review at which the customer and the site
  owners decide whether the deployment and its control scope are approved.
- **Report trace:** §23 Stage 9 exit gate; §22.3.
- **Inputs / prerequisites:** P9.01–P9.27 complete (or explicitly waived — see below); P9.12
  authority tracker; P9.27 register.
- **Steps:**
  1. Assemble the acceptance pack: scope (P9.01), security assessment (P9.07), D6 closure
     (P9.06), HIL report (P9.15), field acceptance results (P9.17), control-scope
     document (P9.26), limitations/risks register (P9.27), procedures (P9.22, P9.23).
  2. Hold the review with the customer, the site owners, and Neryva (product, security,
     power systems, platform). Record attendees, the evidence presented, and the questions
     raised with their answers.
  3. Record the decision per site: approved for the stated control scope / approved with
     conditions (conditions listed, with owners and dates) / not approved.
  4. Any waiver of an incomplete subtask must be explicit, signed by the customer, and
     recorded with its rationale and expiry — a silent gap is not a waiver.
  5. If a site is not approved, record what must change and which subtasks re-open; do
     not record the phase as complete.
- **Deliverable / output:** Acceptance review record: pack contents, attendees, decision
  per site, conditions, waivers (if any).
- **Acceptance criteria / exit check:** (a) the review was held with the customer and site
  owners present; (b) every site has a recorded decision; (c) every condition has an owner
  and a date; (d) any waiver is signed, reasoned, and time-bounded.
- **Owner role:** Product (chair); customer (decision).
- **Dependencies:** P9.01–P9.27; P9.12. Blocks P9.29, P9.30.
- **Verification method:** Inspect the signed review record; confirm the decision per site
  matches the evidence pack (no site approved without its field acceptance record).
- **Risks / failure modes:** Acceptance granted on a slide deck instead of the evidence
  pack; a condition accepted verbally and never recorded; a site approved while its HIL
  or field acceptance record is still open.

### P9.29 — Applicable-authority approval pack

- [ ] **P9.29 Applicable-authority approval pack**

- **Objective:** Submit to each applicable authority the pack it needs to approve the
  deployment and its control scope, and track each submission to a decision.
- **Report trace:** §23 Stage 9 exit gate ("applicable authorities approve"); §22.3.
- **Inputs / prerequisites:** P9.12 tracker; P9.28 acceptance decisions; P9.26 control-scope
  document.
- **Steps:**
  1. For each authority in the P9.12 tracker, assemble the pack it requires (scope,
     interconnection standard/profile, HIL evidence where relevant, control scope,
     customer acceptance record). Do not send an authority a pack written for a different
     authority's requirements.
  2. Submit each pack, record the submission reference and date, and track the response.
  3. Record each decision: approved / approved with conditions / not required (with
     basis) / pending with a named follow-up date. A pending item with no follow-up date
     is open, not "in hand".
  4. Where an authority imposes conditions, record them, assign owners, and verify them
     before the affected site operates under the approved scope.
  5. If an authority's approval is a precondition for a site's operation and it is not
     granted, that site does not operate under operator-approved control.
- **Deliverable / output:** Authority approval pack per authority + decision tracker.
- **Acceptance criteria / exit check:** (a) every authority in P9.12 has a submitted pack
  or a recorded not-required basis; (b) every required approval has a recorded decision
  or a named follow-up date; (c) no site operates under operator-approved control while a
  precondition approval for that site is pending; (d) authority conditions are tracked to
  verification.
- **Owner role:** Product (with legal and power systems).
- **Dependencies:** P9.12, P9.28. Blocks P9.30.
- **Verification method:** Inspect submission references and the authorities' responses;
  confirm no site's control scope exceeds its granted approvals.
- **Risks / failure modes:** An authority assumed to have approved because it did not
  object; a pack submitted with a superseded control-scope version; a precondition
  approval pending while the site is already issuing commands.

### P9.30 — Phase 9 exit-gate record and handover to Phase 10

- [ ] **P9.30 Phase 9 exit-gate record and handover to Phase 10**

- **Objective:** Record the Phase 9 exit-gate decision formally and hand Phase 10 a clean,
  evidenced package — including everything still open.
- **Report trace:** §23 Stage 9 exit gate; §22.3.
- **Inputs / prerequisites:** P9.01–P9.41; the completed `CHECKLIST.md` in this directory.
- **Steps:**
  1. Verify every subtask P9.01–P9.41 is checked with its evidence filed, or is covered by
     a signed, time-bounded waiver recorded in P9.28.
  2. Complete `CHECKLIST.md` with the evidence location for each exit-gate item.
  3. Write the exit-gate record: the gate decision (met / met with conditions / not met),
     the per-site control scopes approved, the waivers and conditions carried forward,
     and the open items Phase 10 must not lose (especially P9.27 limitations, P9.28
     conditions, P9.29 authority conditions).
  4. Hand over to Phase 10: the acceptance pack, the control-scope document, the risk
     register, the frozen versions (software, grid model, adapters), and the list of
     evidence locations. Phase 10 consolidates this evidence into the v1.0.0 release; it
     does not re-run the pilot qualification (see Phase 10 `PLAN.md`). Carry forward the
     NIST SP 800-82 Rev. 4 final-text re-check (comment period closes 30 November 2026;
     P10.16 owns it) as a named forward item so it is not lost between phases.
  5. If the gate is not met, record the phase as not complete and the re-entry conditions.
- **Deliverable / output:** Phase 9 exit-gate record + handover pack index for Phase 10.
- **Acceptance criteria / exit check:** (a) the `CHECKLIST.md` is complete with evidence
  locations; (b) the gate decision is recorded with its basis; (c) every carried-forward
  condition/waiver is listed with an owner and a date; (d) the handover pack index names
  the frozen versions.
- **Owner role:** Product (with the Phase 10 lead).
- **Dependencies:** P9.01–P9.41.
- **Verification method:** Independent check of the checklist against the filed evidence;
  confirm the handover index resolves to real files.
- **Risks / failure modes:** A gate declared met while a waiver has expired; handover that
  drops the limitations register, so Phase 10 release notes omit it; frozen versions
  recorded incorrectly, so Phase 10 validates a different build than the pilot ran.


### P9.31 — Forged/replayed/expired/out-of-sequence command rejection at the edge

- [ ] **P9.31 Forged/replayed/expired/out-of-sequence command rejection at the edge**

- **Objective:** Prove, by attack at the real edge, that a forged, replayed,
  expired, or out-of-sequence command is rejected before it reaches any
  adapter — the §17.4 forged/replayed-command threat, tested rather than
  table-topped.
- **Report trace:** §23 Stage 9; §17.4 (forged/replayed command); §18.2
  (edge validates every command); §14.3 (signature, sequence, expiry,
  idempotency).
- **Inputs / prerequisites:** P9.14 lab results; P9.06 D6 closed; P9.09
  certificate inventory; the pilot edge agent and adapters.
- **Steps:**
  1. Replay a captured signed command (same ID, same signature) after it
     was already issued; confirm rejection and audit.
  2. Forge a command (valid shape, invalid/forged signature) and confirm
     rejection and audit.
  3. Send an expired command (valid-until elapsed) and an out-of-sequence
     command (sequence number behind the last accepted one for the asset);
     confirm rejection and audit in each case.
  4. Send a command with a conflicting payload under an already-used
     idempotency key; confirm rejection, not double execution.
  5. Confirm no rejected command in steps 1–4 reached an adapter or a
     device (adapter logs inspected).
- **Deliverable / output:** Edge command-attack test record (attempt →
  outcome → audit reference) per case.
- **Acceptance criteria / exit check:** (a) every attempt is rejected and
  audited; (b) zero attempts reach an adapter or device; (c) the tests run
  against the pilot edge build and versions, not a lab-only build.
- **Owner role:** Security (with integration engineering).
- **Dependencies:** P9.14; P9.06; P9.09. Blocks P9.28.
- **Verification method:** Independent re-run of the attack cases by a
  second engineer; inspect adapter logs for the rejected command IDs.
- **Risks / failure modes:** Tests run only in the lab while the field edge
  differs; a replay rejected by the control plane but accepted by the edge
  (the edge is the boundary that must reject it).

### P9.32 — Audit integrity and edge audit-buffer proof

- [ ] **P9.32 Audit integrity and edge audit-buffer proof**

- **Objective:** Prove pilot audit records are tamper-evident and complete,
  including the §18.1 local audit buffer's upload after a central outage —
  so the evidence the acceptance decision rests on cannot be altered or
  silently lost.
- **Report trace:** §23 Stage 9; §17.3 (audit logging); §18.1 (local audit
  buffer); §16.3 (retention); §22.3.
- **Inputs / prerequisites:** P9.07 assessment; P9.24 backup/restore
  rehearsal; P9.17 shadow operation.
- **Steps:**
  1. Define and test the audit-integrity mechanism for pilot audit records
     (append-only, hash-chained, or equivalent): tamper with, delete, and
     reorder records in a test copy and confirm detection.
  2. Verify retention enforcement: audit records are retained per the
     §16.3 per-class periods in P9.10; confirm no retention job deletes
     audit evidence early.
  3. Test the edge audit buffer: take the central platform offline for a
     recorded interval while commands/audit events occur at the edge;
     restore connectivity and confirm every buffered audit event uploads
     exactly once (no loss, no duplication) and in order.
  4. Verify an audit export exists for authority review (P9.29) with the
     integrity evidence attached.
- **Deliverable / output:** Audit-integrity + edge-buffer test record.
- **Acceptance criteria / exit check:** (a) tampering is detected in every
  test case; (b) retention is enforced as recorded in P9.10; (c) the
  outage test shows zero audit-event loss and zero duplication; (d) the
  export exists with integrity evidence.
- **Owner role:** Security / platform engineering.
- **Dependencies:** P9.07, P9.10, P9.24. Blocks P9.28.
- **Verification method:** Independent re-run of the tamper suite and the
  outage test; count uploaded audit events against the edge buffer.
- **Risks / failure modes:** Integrity verified on a copy while the live
  store stays writable; a buffer that uploads on reconnect but drops
  events that occurred during the outage.

### P9.33 — Edge enrollment ceremony and identity lifecycle

- [ ] **P9.33 Edge enrollment ceremony and identity lifecycle**

- **Objective:** Define and test how a pilot edge identity is born: the
  enrollment ceremony, first-boot trust, re-enrollment after compromise,
  and enrollment revocation — the part of identity P9.09 (rotation/
  revocation of existing identities) and P9.13 (enrolment as a checklist
  item) never specify.
- **Report trace:** §23 Stage 9; §18.1 (secure site enrollment and
  identity); §17.3 (mTLS, short-lived credentials); §18.3.
- **Inputs / prerequisites:** P9.05 network design; P9.09 secrets/
  certificate inventory; customer edge platform capability.
- **Steps:**
  1. Define the enrollment ceremony: how the edge obtains its first
     identity (enrollment token or equivalent), who issues it, its
     lifetime, and the channel it travels on.
  2. Require the enrollment token to be single-use and short-lived; test
     reuse of a spent token and use of an expired token — both refused.
  3. Define re-enrollment after compromise: how a compromised edge
     identity is revoked (cross-reference P9.09's measured revocation)
     and how the edge re-enrolls without trusting the compromised
     identity.
  4. Test enrollment on one non-production edge end-to-end before the
     pilot starts, including a failed-enrollment case (wrong token,
     wrong site).
- **Deliverable / output:** Enrollment ceremony record + test record.
- **Acceptance criteria / exit check:** (a) the ceremony is written with
  token lifetime and issuer named; (b) spent/expired token reuse is
  refused; (c) the compromise re-enrollment path is written and tested;
  (d) the non-production enrollment test passed.
- **Owner role:** Security / platform engineering.
- **Dependencies:** P9.05, P9.09. Blocks P9.14.
- **Verification method:** Inspect the enrollment test log; attempt token
  reuse against the pilot enrollment endpoint.
- **Risks / failure modes:** A long-lived enrollment token that becomes a
  standing credential; re-enrollment that trusts the old (compromised)
  identity to vouch for the new one.

### P9.34 — Signed edge-update verification and rollback

- [ ] **P9.34 Signed edge-update verification and rollback**

- **Objective:** Prove pilot edge updates are verified before they are
  applied and rolled back when they fail — §18.3 requires signed and
  verified update bundles, but no pilot subtask tests it.
- **Report trace:** §23 Stage 9; §18.3 (signed and verified update
  bundles); §17.3 (secure boot/update where supported); §21.2.
- **Inputs / prerequisites:** P9.13 commissioning plan; P9.09 certificate
  inventory; the edge update mechanism.
- **Steps:**
  1. Attempt an edge update with a bad-signature bundle and with a bundle
     signed by an untrusted key; confirm both are refused before any file
     is written, with the refusal audited.
  2. Apply a valid update to one non-production edge and confirm it
     verifies, applies, and reports its new version.
  3. Apply an update that fails mid-apply (e.g. interrupted download,
     failing post-update health check) and confirm rollback to the
     previous version with the rollback audited and the edge still
     operational.
  4. Record the update policy for the pilot: who approves an update, the
     staging order across sites, and the freeze window during acceptance
     testing (P9.16/P9.17).
- **Deliverable / output:** Edge-update verification + rollback test
  record + pilot update policy.
- **Acceptance criteria / exit check:** (a) bad-signature and untrusted-key
  bundles are refused before any write; (b) a valid update applies and
  reports its version; (c) a failed update rolls back with the edge
  operational; (d) the update policy is written.
- **Owner role:** Platform engineering.
- **Dependencies:** P9.13. Blocks P9.17.
- **Verification method:** Inspect the update and rollback logs on the
  test edge; confirm the version reported after rollback.
- **Risks / failure modes:** Verification that runs after files are already
  written; a rollback that restores the binary but not the configuration.

### P9.35 — §17.5 safety-invariant map for the pilot

- [ ] **P9.35 §17.5 safety-invariant map for the pilot**

- **Objective:** Map each of the ten §17.5 safety invariants to the named
  pilot subtask(s) and test(s) that prove it at the deployment, and close
  the invariants with no live test.
- **Report trace:** §23 Stage 9; §17.5 (invariants 1–10); §14.3; §12.5.
- **Inputs / prerequisites:** P9.07, P9.14, P9.15, P9.17, P9.18, P9.19,
  P9.20, P9.26.
- **Steps:**
  1. Write the map: invariant → enforcing configuration/code → proving
     test (subtask + evidence location). Every invariant must have at
     least one row; an invariant with no test is recorded as open.
  2. Close the gaps found by this review: invariant 1 (a command without
     a valid asset/site identity and current command context is refused)
     — test a command carrying a wrong-site identity and a command built
     on a superseded context; both must be rejected.
  3. Verify the remaining invariants have live tests, not design
     statements — in particular invariant 3 (network feasibility evidence
     for the declared operating profile, P9.03/P9.19) and invariant 6
     (no unapproved protocol adapter or target, P9.04/P9.26).
  4. File the map in the acceptance pack (P9.28); P9.28 must not approve
     a site while any row is open.
- **Deliverable / output:** Invariant→test map for the pilot.
- **Acceptance criteria / exit check:** (a) all ten invariants mapped with
  a named test; (b) the invariant-1 wrong-identity and superseded-context
  tests pass (rejection); (c) invariants 3 and 6 have live tests cited.
- **Owner role:** Security (with power systems).
- **Dependencies:** P9.07, P9.14, P9.19. Blocks P9.28.
- **Verification method:** Reviewer traces each invariant to its test run;
  any row without a run fails.
- **Risks / failure modes:** A map citing design documents instead of test
  runs; an invariant "covered" by a subtask that never actually ran at
  the pilot.

### P9.36 — Live incident drill: forged-command containment

- [ ] **P9.36 Live incident drill: forged-command containment**

- **Objective:** Execute, live (not as a tabletop), the containment of a
  forged/replayed command attempt at the pilot — proving the P9.23
  procedure works when it is run, not only when it is read.
- **Report trace:** §23 Stage 9; §17.3 (incident response and customer
  notification); §17.4 (forged/replayed command).
- **Inputs / prerequisites:** P9.23 procedure + tabletop record; P9.31
  attack-test record; P9.17 shadow operation.
- **Steps:**
  1. Inject a forged-command attempt in a controlled way at the pilot
     (using the P9.31 attack cases as the inject).
  2. Execute the P9.23 procedure live: detection, first responder,
     escalation, customer notification within the recorded deadline, and
     the containment action — revert the affected site to shadow or local
     control.
  3. Preserve evidence before remediation (audit records, telemetry
     snapshots, command records) per P9.23 step 3; confirm nothing was
     destroyed by the response.
  4. Record the drill outcome, timings (detection to containment,
     notification time vs deadline), and corrections to the procedure.
- **Deliverable / output:** Live incident-drill record.
- **Acceptance criteria / exit check:** (a) the attempt was detected and
  contained by the procedure as written; (b) the site was reverted to
  shadow/local control; (c) notification met the recorded deadline;
  (d) evidence was preserved before remediation; (e) procedure corrections
  are applied.
- **Owner role:** Security (with product/operations and the customer).
- **Dependencies:** P9.23, P9.31. Blocks P9.28.
- **Verification method:** Inspect the drill record for timings and the
  customer notification; confirm the reversion in the site's mode record.
- **Risks / failure modes:** A drill that "passes" because the inject was
  never actually delivered; containment that reverts the wrong site.

### P9.37 — Adapter conformance suites executed in the pilot configuration

- [ ] **P9.37 Adapter conformance suites executed in the pilot configuration**

- **Objective:** Run, in the pilot configuration, the conformance/
  interoperability suite P9.04 names for each adapter — so no adapter
  reaches field acceptance on implementation alone (§2.3: implementing an
  adapter is not a conformance claim).
- **Report trace:** §23 Stage 9; §2.3; §19.2(3) (contract tests); §5.3.
- **Inputs / prerequisites:** P9.04 integration design (suite per adapter);
  P9.14 lab environment; adapter/vendor documentation.
- **Steps:**
  1. For each pilot adapter, run its named conformance/interoperability
     suite against the pilot adapter version, profile/variant, and
     firmware version — in the lab first, then against the pilot
     configuration where the suite supports it.
  2. Record per-suite results: cases run, passed, failed, and not
     applicable (with a reason for each not-applicable case).
  3. File every failure with a disposition (fixed and re-run / accepted
     limitation in P9.27 / blocks the pilot); a failed case with no
     disposition blocks P9.28.
  4. Record what the suite does *not* establish: passing a suite is not
     a certification claim for the adapter, the inverter, or the site
     (§11.3, §2.3).
- **Deliverable / output:** Per-adapter conformance execution report.
- **Acceptance criteria / exit check:** (a) every pilot adapter has a run
  of its named suite against the pilot versions; (b) every case has a
  result or a reasoned not-applicable; (c) every failure has a
  disposition; (d) no certification wording appears in the report.
- **Owner role:** Integration engineering.
- **Dependencies:** P9.04, P9.14. Blocks P9.17, P9.28.
- **Verification method:** Inspect the suite run logs against the pilot
  adapter/firmware versions; spot-check one failed case's disposition.
- **Risks / failure modes:** A suite run against a different adapter
  version than the pilot's; a suite "passed" by skipping its failing
  cases without recording them.

### P9.38 — Pilot shadow replay: §24 step 10 at the pilot

- [ ] **P9.38 Pilot shadow replay: §24 step 10 at the pilot**

- **Objective:** Replay a recorded shadow period from stored inputs and
  prove the pilot reproduces the same proposed actions within tolerance —
  the §24 step 10 replay proof, applied at the pilot rather than only in
  the lab.
- **Report trace:** §23 Stage 9; §24 step 10; §19.2(5) (deterministic
  scenario tests); §21.4 (read-only/shadow).
- **Inputs / prerequisites:** P9.17 shadow operation records; P9.18
  telemetry verification; frozen versions per P9.16.
- **Steps:**
  1. Select a recorded shadow period (at least one full operating day, or
     the longest continuous shadow run if shorter) with its stored inputs:
     telemetry snapshots, grid-model version, schedule inputs, seeds,
     and software versions.
  2. Replay the period from the stored inputs alone, on the same frozen
     versions; compare the proposed actions against the originals within
     a tolerance recorded *before* the replay.
  3. Record every divergence beyond tolerance with its cause (nondeterminism
     source, input difference, version difference); a divergence with no
     cause is an open finding, not a rounding note.
  4. File the replay record with the field acceptance evidence (P9.17).
- **Deliverable / output:** Pilot shadow-replay record.
- **Acceptance criteria / exit check:** (a) the replay period, inputs, and
  versions are recorded; (b) proposed actions reproduce within the
  pre-recorded tolerance; (c) every beyond-tolerance divergence has a
  recorded cause.
- **Owner role:** Integration engineering.
- **Dependencies:** P9.17, P9.18. Blocks P9.28.
- **Verification method:** A second engineer replays the same period from
  the record alone and confirms the comparison.
- **Risks / failure modes:** A replay that reuses the original outputs
  instead of recomputing them; a tolerance fitted after seeing the
  divergence.

### P9.39 — Pilot performance and load test

- [ ] **P9.39 Pilot performance and load test**

- **Objective:** Prove the pilot deployment handles its real load: telemetry
  ingest rate, scheduling runtime, query latency, command fan-out, and
  recovery — §19.2(8), which no P9 subtask previously covered at pilot
  scale (P7.26 measures the plugin host only, in the lab).
- **Report trace:** §23 Stage 9; §19.2(8) (performance/load tests); §19.4
  (platform metrics: p95/p99 latency, throughput, recovery).
- **Inputs / prerequisites:** P9.01 scope (pilot scale: sites, assets,
  telemetry rate); P9.14 lab results; P9.17 shadow operation.
- **Steps:**
  1. Define the pilot load profile from P9.01: telemetry ingest rate,
     schedule frequency, concurrent operators, command fan-out per
     dispatch.
  2. Measure at pilot scale: ingest latency (p95/p99), scheduling runtime,
     query latency, command fan-out time, and recovery time after a
     control-plane restart — each with the pilot versions and conditions
     recorded.
  3. Measure the degraded case: ingest at the profile rate while one
     worker/solver is slow or down; record behaviour (queue depth,
     backpressure, rejection) — never silent drops.
  4. Record results as measured values; do not extrapolate beyond the
     tested profile or claim capacity for a larger fleet.
- **Deliverable / output:** Pilot performance/load test report.
- **Acceptance criteria / exit check:** (a) measurements exist for every
  item in step 2 at the defined pilot load; (b) the degraded case is
  recorded with queue/backpressure behaviour; (c) no measurement is
  extrapolated beyond the tested profile.
- **Owner role:** Platform engineering.
- **Dependencies:** P9.01, P9.14. Blocks P9.28.
- **Verification method:** Re-run the load test and compare against the
  recorded numbers within stated tolerance.
- **Risks / failure modes:** Lab numbers presented as pilot numbers; a
  load test that measures the simulator instead of the pilot deployment.

### P9.40 — HIL-to-field traceability map

- [ ] **P9.40 HIL-to-field traceability map**

- **Objective:** Map every field-acceptance case to the HIL/lab evidence
  that de-risks it, reusing the same disturbance profiles and seeds across
  stages — so no FAT case rests on no upstream evidence.
- **Report trace:** §23 Stage 9; §19.1 (HIL track); §19.2(10); RESEARCH.md
  §4 (same disturbance profiles and seeds at each stage).
- **Inputs / prerequisites:** P9.15 HIL report; P9.14 lab report; P9.16
  FAT plan.
- **Steps:**
  1. Build the map: FAT case → HIL case(s) → lab case(s) → simulation
     case(s), with the disturbance profile and seed used at each stage.
     A FAT case with no upstream row is recorded as unmapped, not as
     covered.
  2. Reuse the same disturbance profiles and seeds across stages where
     the stage supports them, so evidence is comparable (RESEARCH.md §4);
     record where a stage could not reuse a profile/seed and why.
  3. For each unmapped FAT case, either add the upstream evidence (re-run
     the HIL/lab case) or record a signed customer acceptance of the gap
     in P9.27 — never leave it blank.
  4. File the map with the FAT plan (P9.16) before P9.17 executes.
- **Deliverable / output:** HIL-to-field traceability map.
- **Acceptance criteria / exit check:** (a) every FAT case has an upstream
  row or a signed accepted gap; (b) disturbance profiles/seeds are reused
  across stages where supported, with exceptions recorded; (c) the map is
  filed before FAT execution.
- **Owner role:** Integration engineering (with power systems).
- **Dependencies:** P9.14, P9.15, P9.16. Blocks P9.17.
- **Verification method:** Pick three FAT cases and trace them to their
  HIL/lab evidence; confirm the profiles/seeds match.
- **Risks / failure modes:** A map built after FAT execution, back-fitted
  to the results; a profile reused by name while its parameters differ
  between stages.

### P9.41 — Scenario-regression suite re-run on the pilot configuration

- [ ] **P9.41 Scenario-regression suite re-run on the pilot configuration**

- **Objective:** Run the §22.2 scenario-regression suite against the pilot
  configuration (pilot adapters, grid model, versions) and record the
  result — a regression that only manifests in the pilot configuration
  must be caught before field acceptance, not after it.
- **Report trace:** §23 Stage 9; §22.2 (scenario regression suite); §19.2(5).
- **Inputs / prerequisites:** P9.03 grid model; P9.04 integration design;
  P9.14 lab results; the suite definition (P7.43 and the Phase 1–6 suite
  artifacts it builds on).
- **Steps:**
  1. Assemble the pilot-configuration suite run: the regression scenarios
     applicable to the pilot (deterministic scenarios, command-lifecycle
     transitions, fault-injection subset, stale/missing telemetry cases)
     executed against the pilot's adapters, grid model, and frozen
     versions.
  2. Record per-scenario pass/fail with the versions and seeds used; a
     failure is dispositioned (fixed and re-run / accepted limitation in
     P9.27 / blocks the pilot).
  3. Confirm the suite version run is recorded, so a later re-run is
     comparable.
- **Deliverable / output:** Pilot-configuration scenario-regression run
  record.
- **Acceptance criteria / exit check:** (a) every applicable scenario has
  a recorded result; (b) every failure has a disposition; (c) the suite
  version is recorded.
- **Owner role:** Integration engineering.
- **Dependencies:** P9.03, P9.04, P9.14. Blocks P9.28.
- **Verification method:** Re-run a sample of scenarios from the record
  and compare outcomes.
- **Risks / failure modes:** A suite run against lab adapter versions
  instead of pilot versions; scenarios skipped as "not applicable" without
  a reason.

---

## Traceability note

Every subtask above traces to §23 Stage 9 and at least one supporting section. Where the
report is silent on a pilot detail (for example, the exact HIL platform — the report says
only "vendor/partner HIL platform", §19.1), this ledger requires the choice to be recorded
with its basis rather than assuming a default. Where a subtask depends on an open decision
(D1–D3, D5, D6), the subtask records the interim position and the condition that would
change it; it does not describe the decision as made (`.agent/DECISIONS.md` rule).
