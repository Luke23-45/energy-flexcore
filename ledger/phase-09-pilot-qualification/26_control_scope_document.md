<!-- P9.26 — Control-scope document and operator-approved configuration. DOCUMENT TEMPLATE ONLY: the release ladder, §21.4 mode rows, promotion rule, and §5.7 never-granted policy are quoted VERBATIM from the report and the kernel; the control-scope document, the operator-approved configuration specification, the deployed-configuration verification record, and the per-site promotion decisions are ALL EMPTY/UNSIGNED — the document is prepared, NOT approved; no site is promoted; no field write authority is granted. Bounded automation is excluded everywhere. -->
# P9.26 — Control-Scope Document and Operator-Approved Configuration (Prepared Document Template — NOT APPROVED, nothing configured, no site promoted)

Date: 2026-11-14. Subtask: P9.26 (`ledger_phase_9.md` §P9.26).
Owner role: Product (scope) + platform engineering (configuration
verification). Report trace: §23 Stage 9; §21.4 (control-authority
deployment modes); §22.3 (release gates; "the target control-authority
mode in §21.4 has passed its own qualification"); §5.7 (plugin
authority model). Inputs: P9.17 shadow acceptance (**NOT EXECUTED —
no shadow review has ever been held**), P9.18–P9.21 (**UNRUN**),
P9.01 scope (**UNSIGNED**), P9.06 D6 closure (**OPEN**),
P9.08 approval chains (`08_identity_access_approval.md` §4 —
DESIGN, no person assigned).

## 1. Honest status — the plain statement

**No control-scope document has been written for any real site (no site
exists), no deployment configuration has been inspected, no site is
promoted to any mode, and no operator-approved configuration has been
signed by anyone.** This record PREPARES the document and the
configuration specification with every value EMPTY/UNSIGNED; the
operator signature that would make any of it effective is a HUMAN GATE
that has not happened and is not scheduled (`ENTRY_NOTE.md` boundary 3:
the control-scope document is a design with the operator-approval gate
recorded, not an approval). Nothing in Phase 9 grants field write
authority. What this record fixes in writing:

- the release-ladder position, quoted verbatim (§2);
- the §21.4 mode table and promotion rule, quoted verbatim (§3);
- the control-scope DOCUMENT template — per site/asset class, all
  cells EMPTY (§4);
- the operator-approved configuration SPECIFICATION — what must be
  configured AND signed before operator-approved mode may be enabled,
  every value EMPTY/UNSIGNED (§5);
- the approval gate and promotion path, building on the P9.17
  shadow-review ordering and the P9.08 approval chains (§6);
- the never-granted list — verbatim from the kernel
  `NEVER_GRANTED_PERMISSIONS` discipline (§7); and
- the explicit bounded-automation exclusion (§8).

## 2. The release ladder (verbatim — report §2.1)

The pilot is operator-approved control AT MOST. The ladder this ceiling
rests on, quoted verbatim from the report (§2.1, "Release definition";
the same ladder appears in `AGENTS.md` §4):

> | Release | Product capability | Control status |
> |---|---|---|
> | **v0.1.0 — FlexCore Lab (first public release)** | Canonical asset/site models, simulator integration, telemetry ingestion, asset registry, portfolio views, scenario replay | Simulation only; no device writes |
> | **v0.2.0 — Flexibility & Scheduling** | BESS/PV/load/EVSE models, flexibility envelopes, deterministic rolling-horizon scheduling, schedule comparisons | Simulation only |
> | **v0.3.0 — Dispatch Assurance** | Resource validation, network feasibility checks, command lifecycle, operator approvals, audit trail | Simulation and shadow mode |
> | **v0.4.0 — Site Integration** | First site adapter through OpenEMS; selected telemetry/protocol adapters; outbound edge connection | Read-only or test-lab control by default |
> | **v0.5.0 — Plugin SDK** | Versioned plugin contracts, plugin manifest, trusted built-in modules, constrained WASM plugin host, examples | Simulation/shadow; no plugin can bypass dispatch gates |
> | **v1.0.0 — Pilot-ready FlexCore** | Stable operator workflow, reproducible benchmark, deployment/upgrade model, security review, site integration test harness | Operator-approved control is the default; production dispatch only after customer-specific acceptance and commissioning |
> | **v1.x — Advanced coordination** | DMPC/ADMM, expanded market adapters, topology-aware optimization research, portfolio analytics | Per-feature qualification; bounded automation may be granted per deployment |
> | **v2.x — Advanced grid intelligence** | GNN acceleration, safe MARL research, dynamic stability research, broader utility/DSO integrations | Shadow/simulation first; no automatic field authority by default |

And the standing qualification, verbatim (§2.1 closing sentence):
*"A version number does not imply regulatory certification or
operational qualification. The control-authority mode actually enabled
at a site is governed by §21.4 and the release gates in §22.3."*

The §0.x line is simulation-only (§0: *"The **0.x line is
simulation-only**"*). Operator-approved control is the **v1.0.0**
mode: the pilot's operator-approved ceiling is therefore not reachable
before the v1.0.0 gate evidence exists — see §5's gate list.

## 3. The §21.4 mode rows and promotion rule (verbatim)

> | Mode | Authority | Writes to devices | Default use |
> |---|---|---|---|
> | Simulation | None | No | v0.1.0; development, demonstration, benchmarking |
> | Read-only / shadow | None; computes and compares proposed actions against measurements | No | Early pilots and commissioning |
> | Operator-approved | Each command requires an authorized human approval | Yes, after approval | v1.0.0 default |
> | Bounded automation | Pre-approved policy envelope with limits, expiry, and automatic fallback | Yes, inside the envelope only | Post-1.0, per deployment, site, and asset class after qualification |

> "Control-authority mode is a deployment and per-site setting, not
> something a plugin can select. A deployment may run mixed modes by
> site or asset class. Promotion to a higher-authority mode requires
> the release gates in §22.3 plus site-specific acceptance."

For the pilot the ceiling is the row whose Default use is
"v1.0.0 default": **operator-approved** — and only after §22.3 plus
site-specific acceptance. Bounded automation's own row places it
"Post-1.0" — outside the pilot entirely (§8).

## 4. Control-scope document — TEMPLATE (every value EMPTY)

One document per deployment; one row per site; asset-class rows within
each site. The document is effective ONLY when every signature in §6.2
exists. Filling any cell from anything other than the signed
decision is fabrication.

### 4.1 Site/control-scope table (ALL CELLS EMPTY)

| Field | Value |
|---|---|
| Deployment identifier (signed P9.01 freeze) | `<EMPTY>` |
| Site ID | `<EMPTY>` |
| Asset class(es) in scope | `<EMPTY>` |
| Authorised mode per §21.4 (pilot ceiling: read-only/shadow, then operator-approved — see §6) | `<EMPTY>` |
| Asset set (asset IDs per P9.02 reconciliation) | `<EMPTY>` |
| Power limits per asset class (MW; active power, `P > 0` = export at the point of connection) | `<EMPTY>` |
| Energy limits per asset class (MWh) | `<EMPTY>` |
| Reactive-power sign convention — **declared per grid model and device profile, never assumed** | `<EMPTY>` |
| Validity window (start, UTC) | `<EMPTY>` |
| Expiry / review date of the scope | `<EMPTY>` |
| Approved adapters (adapter + version + profile, per P9.04) | `<EMPTY>` |
| Grid-model version in force (P9.03) | `<EMPTY>` |
| Excluded asset classes / services | `<EMPTY>` |
| Customer signature / date | `<EMPTY — UNSIGNED>` |
| Neryva product-owner signature / date | `<EMPTY — UNSIGNED>` |
| Site-owner signature / date (§22.3 "site owner approval") | `<EMPTY — UNSIGNED>` |

### 4.2 Scope document rules (structural)

| # | Rule |
|---|---|
| S-1 | The scope NEVER names a mode above operator-approved; a document row reading "bounded automation" is INVALID by construction (§8) |
| S-2 | Every value is a deployment decision recorded here BEFORE enabling anything; a site operating outside its recorded scope is a FINDING, and the deployment is misconfigured by definition |
| S-3 | Limits use the canonical units (MW, MWh; reactive power declared per grid model and device profile) — a limit without a declared reactive-power sign is incomplete and cannot be approved |
| S-4 | The scope carries an expiry/review date; an expired scope reverts the site to read-only/shadow until re-issued — the enforcement of this is a deployment composition item (honest gap: no code enforces scope expiry today) |
| S-5 | Scope-creep control (P9.01 §4 step 7): any site/asset/service addition requires re-issuing the frozen scope, never an in-place edit |

## 5. Operator-approved configuration specification (what must be configured AND signed; every value EMPTY/UNSIGNED)

Operator-approved mode ("Yes, after approval" — §21.4) requires ALL of
the following configured in the DEPLOYED system and signed before
enablement. The verification method is inspection of the RUNNING
configuration, never the design document (ledger step 2).

| # | Configuration item | Source of truth | Configured? | Signed? |
|---|---|---|---|---|
| C-1 | Control-authority mode set per site in the DEPLOYMENT configuration (§21.4: a deployment/per-site setting) | §4 document, deployed config manifest | `<EMPTY>` | `<EMPTY>` |
| C-2 | Site list in the deployed configuration matches §4 field-for-field | §4 document | `<EMPTY>` | `<EMPTY>` |
| C-3 | Asset list per site matches §4 field-for-field | §4 document + P9.02 registry | `<EMPTY>` | `<EMPTY>` |
| C-4 | Power/energy limits deployed match §4 field-for-field | §4 document | `<EMPTY>` | `<EMPTY>` |
| C-5 | Approval roles assigned per the P9.08 chains: `command.approve` held only by mapped `TenantAdmin` principals at site/asset scope; proposer ≠ approver separation live (P4.13 gate) | `08_identity_access_approval.md` §4.1; `crates/flexcore-authz/src/phase4_policies.rs` | `<EMPTY>` | `<EMPTY>` |
| C-6 | Qualification register: every principal holding `command.approve` passed the P9.21 practical exercise (P9.08 §7 rule) | `21_operator_training.md` register | `<EMPTY>` | `<EMPTY>` |
| C-7 | IdP/identity resolution composed (real resolver, fail-closed posture) — no `DenyAllResolver`-only deployment can be operator-approved because no real approver identity exists | `08_identity_access_approval.md` §8 | `<EMPTY>` | `<EMPTY>` |
| C-8 | §22.3 gate evidence for the target control-authority mode: *"the target control-authority mode in §21.4 has passed its own qualification"* — plus the full §22.3 list (command transitions tested, authz/tenant tests, stale-telemetry handling, edge degraded modes, plugin boundaries, backup/restore, cyber and operational risk review) and *"a production field pilot additionally requires site owner approval, device/vendor integration tests, local safety review, commissioning, and any applicable market/interconnection qualification"* | §22.3; the P9.28 acceptance pack | `<EMPTY>` | `<EMPTY>` |
| C-9 | Site-specific acceptance for THIS site: the P9.17 shadow review signed (O-3) and the P9.28 per-site decision recorded | `17_field_acceptance_execution.md` §6; P9.28 record | `<EMPTY>` | `<EMPTY>` |
| C-10 | No automation envelope present anywhere in the configuration (ledger acceptance (d)) — verified by inspection, including no residual research/lab profile (§5.8 `research-gnn`/`research-marl`/`sim-lab` carry no dispatch authority by design, but their PRESENCE in a pilot deployment is itself a finding to disposition) | Ledger step 5; §8 below | `<EMPTY>` | `<EMPTY>` |
| C-11 | D6 closure signed for this deployment (precondition for any pilot activity, read-only/shadow included) | `06_ot_it_boundary_d6.md` | `<EMPTY>` | `<EMPTY>` |

**Every Configured? and Signed? cell is EMPTY/UNSIGNED.** A single
unconfigured or unsigned row blocks operator-approved mode for the
whole deployment; partial enablement is a finding, not a partial
approval.

## 6. The approval gate and the promotion path

### 6.1 The operator signature is the human gate

The §4 signatures and every §5 Signed? cell are HUMAN decisions. This
record is **prepared, NOT approved**; the approval (when a real
customer, site, and deployment exist) is granted in the P9.28
acceptance review and the per-site promotion decision — never by an
agent, never by this document's existence, never by a plugin
(§21.4 verbatim: mode is "not something a plugin can select").

### 6.2 The promotion path per site (structural ordering; a violation is a finding, never repaired)

| # | Gate | Source |
|---|---|---|
| P-1 | P9.01 freeze signed; P9.06 D6 closure signed for the deployment | P9.01 §3.I; `06_ot_it_boundary_d6.md` §6 |
| P-2 | Commissioning complete with the C8.3 shadow-entry sign-off SIGNED (`13_commissioning_plan.md` §4 C8.3 — D6 closure is a shadow-entry precondition) | `13_commissioning_plan.md` |
| P-3 | The site runs read-only/shadow; ALL shadow cases dispositioned (P9.17 O-2) | `17_field_acceptance_execution.md` §2.3 |
| P-4 | The shadow review SIGNED — customer reviewed shadow results including tracking error and every rejected command with its reason (P9.17 O-3) | `17_field_acceptance_execution.md` §6 |
| P-5 | Promotion REQUESTED per this document (the request never grants itself — P9.17 O-4); the request cites the §4 control-scope document and the §22.3 evidence (C-8) | `17_field_acceptance_execution.md` §2.3 O-4 |
| P-6 | Promotion GRANTED by the customer/site decision (P9.28 per-site decision) — recorded per site in the promotion-decision table below | P9.28 |
| P-7 | ANY failed operator-approved case ⇒ the site REVERTS to shadow, the reversion is RECORDED, and no further commands issue until a re-accepted promotion (P9.17 O-6) | `17_field_acceptance_execution.md` §2.3 O-6 |

### 6.3 Per-site promotion decisions (record — EMPTY)

| Site ID | Shadow-review ref (signed) | §22.3 evidence ref | Control-scope document version cited | Promotion decision (granted / with conditions / refused) | Decision date | Decision-maker (customer/site owner) |
|---|---|---|---|---|---|---|
| — | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` | `<EMPTY>` |

**No promotion decision exists. No site has ever been promoted.** The
table exists so the promotion is always a RECORDED decision with a
named decision-maker — "a site promoted on a verbal approval" (the
ledger's named failure mode) has nowhere to hide.

## 7. The never-granted list (verbatim from the kernel discipline)

The kernel's manifest validation refuses these BEFORE composition,
lifecycle, or any broker exists to mis-grant them, and re-refuses at
grant time (P7.11); profile ceilings also refuse them
(`PROFILE_PERMISSION_CEILING_NEVER_GRANTED`):

```text
crates/flexcore-kernel/src/manifest.rs:
/// Permissions NEVER granted to a plugin (§5.7 never-granted rows). A
/// manifest requesting one is rejected here — before composition, lifecycle,
/// or any broker exists to mis-grant it (P7.11 re-refuses at grant time).
pub const NEVER_GRANTED_PERMISSIONS: [&str; 4] = [
    "dispatch.authorize",
    "dispatch.issue",
    "secret.read",
    "network.any",
];
```

And the report §5.7 authority model, verbatim rows:
*"Authorize dispatch | `dispatch.authorize` | Never granted to general
plugins"* and *"Issue physical command | `dispatch.issue` | Owned by
protected dispatch service and protocol adapter under a signed
authorization token"*; §5.7 closing sentence: *"The platform kernel
owns final authorization and dispatch state transitions. An optimizer,
GNN, MARL policy, or external plugin cannot call device protocols
directly."*

Consequences the pilot configuration inherits (not new policy):

| Rule | Basis |
|---|---|
| No plugin can select or raise the control-authority mode (§21.4) — and no plugin path can grant `dispatch.authorize`/`dispatch.issue` at all (refusal is manifest-level, before a broker exists) | `manifest.rs` above; `profile.rs` ceiling; §5.7 |
| The authorization vocabulary itself never parses `command.issue` / `dispatch.authorize` / `dispatch.issue` — unknown ids deny with `DenyReason::UnknownAction` | `crates/flexcore-authz/src/action.rs`; P9.08 gap G5 (permanent exclusion) |
| The mode-change negative test (ledger verification method: "attempt a mode change via a plugin path and confirm it is refused") is the §9 execution cell N-1 | This record §9 |
| FlexCore is a supervisory orchestration layer — no protection-grade or primary-response authority is granted to anyone by any scope document row (AGENTS.md §3 rules 2–3) | AGENTS.md §3; §0.3 |

## 8. The bounded-automation exclusion (explicit, verbatim bases)

Bounded automation is NOT enabled anywhere in the pilot, and the
configuration contains NO automation envelope. Verbatim bases:

- §21.4 mode row: *"Bounded automation | Pre-approved policy envelope
  with limits, expiry, and automatic fallback | Yes, inside the
  envelope only | **Post-1.0, per deployment, site, and asset class
  after qualification**"*.
- `ENTRY_NOTE.md` boundary 3: *"Bounded automation is post-1.0.
  Nothing in Phase 9 grants field write authority; the control-scope
  document (P9.26) is a design with the operator-approval gate
  recorded, not an approval."*
- The §2 ladder rows v1.x/v2.x place bounded automation at
  "Per-feature qualification" — never a pilot-period capability.

Configuration consequence (to be verified by C-10 inspection): no envelope,
no pre-approved policy set, no automatic-fallback automation setting
exists in the pilot deployment. §17.5 safety-invariant discipline and
the §17.4 "Central outage → site-local fallback and autonomous local
EMS" row refer to the CUSTOMER's local EMS capability, not to a
FlexCore automation envelope (P9.20 R-5 keeps this boundary honest).

## 9. Configuration-verification and negative-test matrix (TEMPLATE — UNRUN)

The deployed configuration is verified against the §4 document
field-for-field (ledger acceptance (a)) by inspection of the RUNNING
configuration; the plugin-path mode-change attempt is executed once
per deployment. Every cell EMPTY — no deployment exists to inspect.

| # | Test | Expected | Executed? | Evidence |
|---|---|---|---|---|
| N-1 | Attempt a mode change (or a mode RAISE) via a plugin path | Refused — mode is a deployment/per-site setting no plugin can select (§21.4); the plugin surface never carries the capability | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N-2 | Attempt a manifest requesting `dispatch.authorize` | Refused `PERMISSION_NEVER_GRANTED` before composition | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N-3 | Attempt a profile granting `dispatch.issue` | Refused `PROFILE_PERMISSION_CEILING_NEVER_GRANTED` | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N-4 | Field-for-field diff of deployed config vs §4 document (mode, sites, assets, limits) | Zero drift; any drift is a FINDING and the deployment is corrected, not the document | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N-5 | Search the deployed configuration for any automation envelope or residual lab/research profile (§5.8) | None present; a residual profile is a FINDING to disposition (ledger risk row) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| N-6 | Verify each operator-approved site has a §6.3 promotion decision dated AFTER its signed shadow review | Every promoted site traces to P-1..P-6 | `<EMPTY — UNRUN>` | `<EMPTY>` |

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and that every §4/§5/§6.3/§9 value is
   EMPTY/UNSIGNED — the document is prepared, NOT approved; no
   promotion exists.
2. Confirm §2 quotes the release ladder verbatim and the ceiling
   argument (operator-approved = the v1.0.0 row; 0.x simulation-only)
   is stated, not implied.
3. Confirm §3 quotes §21.4 verbatim including "not something a plugin
   can select" and "Promotion ... requires the release gates in §22.3
   plus site-specific acceptance".
4. Confirm §5 C-5/C-6/C-7 build on the P9.08 chains (approver =
   TenantAdmin at site/asset only; proposer ≠ approver; qualification
   register) and §6 orders promotion after the signed shadow review
   (P9.17 O-3/O-4).
5. Confirm §7's never-granted list is byte-identical to
   `crates/flexcore-kernel/src/manifest.rs` `NEVER_GRANTED_PERMISSIONS`
   and the §5.7 rows, and §8 excludes bounded automation with verbatim
   citations.
6. Confirm no certification/safety wording appears and the ledger tick
   wording is "implemented, awaiting human build/test".

## 11. Filling commands (for the human executor)

- After a real P9.01 freeze and P9.06 D6 closure: draft the §4 document
  per site/asset class from the signed scope; obtain the three §4
  signatures (customer, Neryva product, site owner).
- Before ANY operator-approved enablement: complete every §5 row
  (configured AND signed); run §9 N-1–N-6 against the RUNNING
  configuration and file the evidence with the CHECKLIST item
  "Control scope documented and enforced".
- Record each promotion in §6.3 only after P-1–P-6; on any
  operator-approved case failure execute §6.2 P-7 and record the
  reversion.
- Carry the §4 S-4 scope-expiry gap and the §9 findings to P9.27 (§12).

## 12. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No control-scope document, no deployment configuration, no promotion decision exists — every ledger acceptance criterion is UNMET and not claimed | §1, §4, §6.3 |
| Configuration-drift risk is structural: the document/deployment match is verified only by human inspection (N-4); no code continuously enforces document==deployed | §9 N-4; the ledger's named risk row |
| Scope expiry is procedural — no code enforces the §4 expiry/review date; an expired scope holds only if a human re-checks it | §4 S-4 |
| Promotion discipline (shadow review before request, customer grant) is enforced by ordering rules whose enforcement at a real pilot is human (P9.17 R-2/R-8 pressure) | §6.2; `17_field_acceptance_execution.md` R-2, R-8 |
| An automation envelope left enabled from a lab profile (the ledger's named failure mode) is prevented only by the N-5 human inspection until then | §9 N-5 |

Status: **WRITTEN-UNRUN** — a document template cannot be "run"; the
signatures, deployed-configuration inspection, plugin-path negative
tests, and promotion decisions are the human gates and have not
happened and are not scheduled. Nothing was configured; no site was
promoted; no pilot exists and none is claimed.
