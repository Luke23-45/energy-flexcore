<!-- P10.15 — Control-authority qualification: operator-approved mode.
     QUALIFICATION RECORD with verdict NOT QUALIFIED: the mode-entry
     requirements (the Stage 9 chain) are ASSEMBLED from the actual Phase 9
     records by citation — every requirement's status is EMPTY/UNSIGNED/
     NOT-MET; the §21.4 mode rows and promotion rule, the never-granted
     list, and the bounded-automation exclusion are quoted VERBATIM; the
     bypass/enforcement/UX verification cells are ALL EMPTY/UNRUN (no
     candidate exists, no deployment exists, no browser run has happened).
     Nothing here grants, enables, or schedules any control authority. -->
# P10.15 — Control-Authority Qualification: Operator-Approved Mode (VERDICT: NOT QUALIFIED — requirements assembled, every status EMPTY/UNSIGNED/NOT-MET)

Date: 2026-11-14. Subtask: P10.15 (`ledger_phase_10.md` §P10.15). Owner role:
Security (with product and platform engineering). Report trace: §22.3 release
gate 11 ("the target control-authority mode in §21.4 has passed its own
qualification"); §21.4; §14. Inputs: P10.01 scope freeze
(`01_release_scope_freeze.md`); the Phase 9 control-scope document P9.26
(`ledger/phase-09-pilot-qualification/26_control_scope_document.md` —
prepared, NOT approved; the configuration specification C-1..C-11 and the
promotion path P-1..P-7 are cited, not redone); P9.08 approval chains
(`ledger/phase-09-pilot-qualification/08_identity_access_approval.md`); the
P10.03 command-transition suite (`03_command_transition_tests.md`,
WRITTEN-UNRUN); P10.04 authz/tenant suite (concurrent sibling, cited by
expected path `04_authz_tenant_tests.md`); P10.08 plugin boundary suite
(`08_plugin_boundary_tests.md`, WRITTEN-UNRUN); P10.13's schema-level gate
ordering (`13_schema_migration_compat.md` §3 T-4, WRITTEN-UNRUN); P10.18
(candidate — does not exist); P10.23 (operator-workflow stability — cited by
expected path). Deliverable: this record (no code, no configuration, no
test file — the qualification evidence class is verification of an
enablement that has not happened).

## 1. Honest status — the plain statement

**Operator-approved mode is NOT QUALIFIED. No command has ever been approved
by an authorized human; no deployment configuration exists; no candidate
exists to run the bypass tests against; no site is promoted to any mode; no
operator is qualified; no control-scope document is signed.** The §22.3
gate-11 requirement ("the target control-authority mode in §21.4 has passed
its own qualification") is **NOT MET** and is recorded as such — the Phase 10
gate exception (ENTRY_NOTE.md) stands. This record assembles the
qualification TARGET (§2), the entry requirements and their EMPTY/UNSIGNED/
NOT-MET statuses (§3–§4), the verbatim governing rules (§5–§7), the
unrun bypass/enforcement/UX verification plan (§8), and the verdict with the
exact evidence that would change it (§9) — which is a list of records that
do not exist, never a promise or a schedule. Nothing in Phase 10 grants
field write authority; the release ladder's v1.0.0 row is a target position,
not an achieved state.

## 2. The qualification target, stated precisely (ledger step 1)

The target mode is **operator-approved**, verbatim from §21.4's mode table
(§5 below): *"Each command requires an authorized human approval"*, *"Yes,
after approval"*, default use *"v1.0.0 default"*. Concretely, per §14.3 and
the P9.08 chains: every command reaches ISSUED only through the frozen chain
PROPOSED → VALIDATED → FEASIBLE → **APPROVED** → ISSUED, where APPROVED is
produced only by the P4.13 authorization gate (`command.approve`,
TenantAdmin-only at site/asset scope, proposer ≠ approver, recorded expiry,
evidence reference) — validation, feasibility, and authorization are three
distinct gates, and no action, grant, or plugin collapses them. Writes to
devices happen only after approval; APPROVED ≠ issued, ISSUED ≠ delivered,
acknowledgement is not physical delivery (AGENTS.md §4).

## 3. The mode-entry requirements (the Stage 9 chain), assembled by citation — every status EMPTY/UNSIGNED/NOT-MET

The requirement rows cite the Phase 9 records that own them; no requirement
is restated, closed, or re-decided here. **Every status is EMPTY, UNSIGNED,
or NOT-MET** because the owning records say so themselves.

| # | Requirement (the Stage 9 chain) | Owning record (cited) | Status |
|---|---|---|---|
| R-1 | Release scope freeze SIGNED (P9.01) | `ledger/phase-09-pilot-qualification/01_pilot_scope_freeze.md`; P9.26 §6.2 P-1 | NOT-MET (Stages 1–9 gates NOT HELD/NOT MET — ENTRY_NOTE) |
| R-2 | D-closures signed for the deployment (D6 OT/IT boundary et al.) | `06_ot_it_boundary_d6.md` (D6 OPEN); P9.26 §5 C-11 | NOT-MET |
| R-3 | Commissioning complete with the C8.3 shadow-entry sign-off SIGNED | `13_commissioning_plan.md` §4 C8.3; P9.26 §6.2 P-2 | NOT-MET |
| R-4 | Shadow acceptance: all shadow cases dispositioned, shadow review SIGNED (customer reviewed tracking error and every rejected command with its reason) | `17_field_acceptance_execution.md` §2.3/§6 (P9.17 NOT EXECUTED); P9.26 §6.2 P-3/P-4 | NOT-MET |
| R-5 | Promotion REQUESTED citing the control-scope document and §22.3 evidence, then GRANTED by the customer/site decision — recorded per site with a named decision-maker | P9.26 §6.2 P-5/P-6, §6.3 (promotion table EMPTY) | NOT-MET |
| R-6 | Authority approvals: site owner approval, local safety review, device/vendor integration tests, any market/interconnection qualification | §22.3 closing paragraph; P9.26 §5 C-9 | NOT-MET |
| R-7 | Control-scope document SIGNED per site/asset class (three signatures) | P9.26 §4 (all signature cells `<EMPTY — UNSIGNED>`) | UNSIGNED |
| R-8 | Operator-approved configuration specified AND signed: mode per site in the DEPLOYED config, site/asset/limit match, approval roles, qualification register, real IdP resolver (fail-closed), no automation envelope anywhere | P9.26 §5 C-1..C-11 (every Configured? and Signed? cell `<EMPTY>`) | EMPTY |
| R-9 | Operators qualified: every principal holding `command.approve` passed the P9.21 practical exercise (approve a valid command, reject an infeasible one with its reason, recognise stale telemetry, execute fallback) — the rule is procedural until implemented (P9.08 §7 honest gap) | `21_operator_training.md` (register EMPTY); P9.08 §7 | NOT-MET |
| R-10 | §22.3 gate evidence for the target mode (this gate) plus the full §22.3 list | P9.26 §5 C-8; this record §8–§9 | NOT-MET (this gate: NOT QUALIFIED) |

## 4. The candidate-configuration checks (ledger steps 2–5) — ALL UNRUN

| # | Ledger step | What would be verified on the candidate | Executed? | Evidence |
|---|---|---|---|---|
| V-1 | Step 2 — approval-gate bypass: expired approval | A command with an expired approval cannot reach ISSUED (§14.3 expiry; the P4.13 gate's recorded expiry; the edge's independent validity-window enforcement) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-2 | Step 2 — approval for a different scope | An approval whose scope does not cover the command's asset/limits cannot authorize it (the scope check compares limits, not just the asset ID — the ledger's named risk) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-3 | Step 2 — approval by an unauthorized role | A non-approver role's approve attempt denies (`NoMatchingRule` — P9.08 N1; no Operator approve grant exists) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-4 | Step 2 — approval after supersede | A SUPERSEDED command cannot be approved into ISSUED (the frozen edge table has no SUPERSEDED→ISSUED edge; the schema-level half is P10.13 T-4, WRITTEN-UNRUN) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-5 | Step 3 — mode enforcement per site | The configured mode is a deployment/per-site setting enforced per site; mixed modes by site/asset class behave as configured | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-6 | Step 3 — no plugin can select or raise the mode | The §5.7/§21.4 rule: refusal is manifest-level (`NEVER_GRANTED_PERMISSIONS`) and profile-level (`PROFILE_PERMISSION_CEILING_NEVER_GRANTED`); the P9.26 N-1 negative test executes on the candidate | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-7 | Step 4 — human-approval UX | The approver sees command + validation/feasibility evidence + scope/limits BEFORE approving (§20.2); a proposed setpoint is never displayed as if executed; the P10.23 workflow-stability check consumes the same candidate build | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-8 | Step 5 — per-site confirmation | The candidate configuration matches the P9.26 §6.3 promotion decisions per site/asset class (there are none — the table is EMPTY) | `<EMPTY — UNRUN>` | `<EMPTY>` |
| V-9 | Step 6 — bounded automation absent | No automation envelope in the release configuration (P9.26 C-10/N-5; §7 below) | `<EMPTY — UNRUN>` | `<EMPTY>` |

Static, unrun anchors that these checks will consume when they run (cited,
not executed): the P10.03 suite's 27 legal / 169 illegal edge matrix
(WRITTEN-UNRUN); the P10.13 T-4 schema-level refusals of PROPOSED→ISSUED and
PROPOSED→APPROVED (WRITTEN-UNRUN); the P10.04 authz/tenant suite and P10.08
plugin-boundary suite (WRITTEN-UNRUN); the P9.08 §9 negative-test matrix
N1–N7 (UNRUN). Zero observed failures are NOT claimed from any of them — an
unrun suite has no observed failures.

## 5. The §21.4 mode rows and promotion rule (verbatim)

> | Mode | Authority | Writes to devices | Default use |
> |---|---|---|---|
> | Simulation | None | No | v0.1.0; development, demonstration, benchmarking |
> | Read-only / shadow | None; computes and compares proposed actions against measurements | No | Early pilots and commissioning |
> | Operator-approved | Each command requires an authorized human approval | Yes, after approval | v1.0.0 default |
> | Bounded automation | Pre-approved policy envelope with limits, expiry, and automatic fallback | Yes, inside the envelope only | Post-1.0, per deployment, site, and asset class after qualification |

> "Control-authority mode is a deployment and per-site setting, not
> something a plugin can select. A deployment may run mixed modes by site or
> asset class. Promotion to a higher-authority mode requires the release
> gates in §22.3 plus site-specific acceptance."

And the standing qualification (report §2.1, verbatim): *"A version number
does not imply regulatory certification or operational qualification. The
control-authority mode actually enabled at a site is governed by §21.4 and
the release gates in §22.3."* The mode is **never plugin-selectable** — no
plugin can select or raise it (§21.4 verbatim above; §5.7; enforced
manifest-level before any broker exists, P7.11).

## 6. The never-granted list (verbatim, from the kernel discipline)

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

And the report §5.7 authority model, verbatim rows: *"Authorize dispatch |
`dispatch.authorize` | Never granted to general plugins"* and *"Issue
physical command | `dispatch.issue` | Owned by protected dispatch service
and protocol adapter under a signed authorization token"*; §5.7 closing
sentence: *"The platform kernel owns final authorization and dispatch state
transitions. An optimizer, GNN, MARL policy, or external plugin cannot call
device protocols directly."* The authorization vocabulary itself never
parses `command.issue` / `dispatch.authorize` / `dispatch.issue` — unknown
ids deny with `DenyReason::UnknownAction` (P9.08 gap G5, permanent
exclusion). No plugin path can grant or raise the control-authority mode.

## 7. The bounded-automation exclusion (explicit, verbatim bases)

Bounded automation is **NOT qualified by this subtask, NOT enabled in the
release configuration, and NOT scheduled anywhere in the v1.0.0 line**.
Verbatim bases:

- §21.4 mode row: *"Bounded automation | Pre-approved policy envelope with
  limits, expiry, and automatic fallback | Yes, inside the envelope only |
  **Post-1.0, per deployment, site, and asset class after qualification**"*.
- The release ladder (§2.1): v1.x places bounded automation at *"Per-feature
  qualification"* — never a v1.0.0 capability; v0.x is simulation-only
  through v0.5.0.
- P9.26 §8: *"Bounded automation is NOT enabled anywhere in the pilot, and
  the configuration contains NO automation envelope."*
- AGENTS.md §4 control-status ladder: Bounded automation is post-1.0, per
  deployment, site, and asset class.
- The P10.30 post-release deferral record (concurrent later wave) owns the
  deferral register; this record only states the exclusion's basis.

Configuration consequence (verified only by V-9's unrun inspection): no
envelope, no pre-approved policy set, no automatic-fallback automation
setting exists in the release configuration; a residual lab/research profile
present in a pilot deployment is a FINDING to disposition (P9.26 N-5), never
a tolerated default.

## 8. The qualification verdict (ledger acceptance criteria — all UNMET, none claimed)

| Acceptance criterion | Status |
|---|---|
| (a) every bypass attempt failed — no command reached ISSUED without a valid, in-scope, unexpired approval | **UNMET — no attempt has been made (V-1..V-4 UNRUN; the suites are WRITTEN-UNRUN)** |
| (b) mode enforcement matches the Phase 9 control-scope document per site | **UNMET — no deployment configuration exists; the document is UNSIGNED (R-7/R-8; V-5 UNRUN)** |
| (c) the approver UX shows evidence before approval | **UNMET — no candidate console build exists; no browser run has happened (V-7 UNRUN; P10.14 record)** |
| (d) no automation envelope is enabled in the candidate configuration | **UNMET as verified — the exclusion is stated (§7), the inspection has not run (V-9 UNRUN)** |

## 9. VERDICT: **NOT QUALIFIED** — and what would change it (evidence, never a promise)

**The verdict is NOT QUALIFIED, recorded on 2026-11-14.** The verdict changes
ONLY when all of the following evidence exists AND is verified by the named
reviewer procedures — none of it exists today; listing it is not a schedule,
a forecast, or a commitment:

1. Every §3 requirement row flips to MET/SIGNED in its OWNING record (the
   P9.26 §4 signatures; the §5 C-1..C-11 Configured? AND Signed? cells; the
   P9.21 qualification register filled; the P9.17 shadow review signed; the
   P9.06 D6 closure signed).
2. A per-site promotion decision exists in P9.26 §6.3, granted AFTER the
   signed shadow review, by a named customer/site decision-maker
   (P-1..P-6 in order).
3. The V-1..V-9 checks execute against a REAL candidate (P10.18) in a REAL
   deployment configuration, every bypass attempt REFUSED with recorded
   evidence, and the independent verification method executes one bypass
   attempt per class.
4. The unrun suites this record cites (P10.03, P10.04, P10.08, P10.13 T-4)
   are RUN and their failures dispositioned — zero observed failures from an
   unrun suite never counts.
5. The §22.3 list completes for the deployment (including gates 9, 10, 12 —
   P10.11/P10.12/P10.13/P10.14/P10.16 run records filled) and the P10.27
   go/no-go records the mode position.

Until then: the release ships (if it ships at all) with **simulation /
read-only (shadow) authority only**; no command reaches a device through any
path this record describes; and any text implying otherwise is a violation
of AGENTS.md rules 2, 4, and 5.

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement and the verdict: **NOT QUALIFIED** stands in
   §9, and every §3 Status cell is EMPTY/UNSIGNED/NOT-MET with its owning
   record cited.
2. Confirm §5 quotes §21.4 verbatim including "not something a plugin can
   select" and "Promotion ... requires the release gates in §22.3 plus
   site-specific acceptance".
3. Confirm §6's never-granted list is byte-identical to
   `crates/flexcore-kernel/src/manifest.rs` `NEVER_GRANTED_PERMISSIONS` and
   the §5.7 rows.
4. Confirm §7 excludes bounded automation with verbatim citations (§21.4 row,
   release ladder, P9.26 §8) and that no envelope is enabled or scheduled.
5. Confirm §8 marks every acceptance criterion UNMET without a run claim,
   and §9's evidence list is stated as conditions, never as a promise.
6. Confirm no certification/safety wording appears and the ledger tick
   wording is "implemented, awaiting human build/test".

## 11. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the unrun suites this record consumes (human build/test gates):
cargo test -p flexcore-domain --test command_transition_release_matrix   # P10.03
cargo test -p flexcore-postgres-adapter --test migration_compat_p10_13   # P10.13 T-4
#     plus the P10.04 authz/tenant and P10.08 plugin-boundary suites when
#     their records name their commands.

# (c) Fill §3 statuses ONLY from the owning records (never restate or close
#     another record's item here).
# (d) Execute V-1..V-9 against a real candidate (P10.18) in a real deployed
#     configuration; fill the Executed?/Evidence cells FROM THE RUN ONLY.
# (e) Re-derive the verdict in §9; a NOT QUALIFIED verdict only changes when
#     every condition in §9's list is met AND verified.
# (f) Hand the result to P10.27 (go/no-go) and P10.25 (security
#     consolidation).
```

## 12. Findings (recorded, not fixed)

- **F-P10.15-1 (the ceiling is structural):** the qualification cannot pass
  before the Stage 9 chain exists — no site, no deployment, no signed
  document. The §22.3 gate-11 requirement is NOT MET and the P10.27 go/no-go
  must carry it as open unless the entire §9 evidence chain is filled.
- **F-P10.15-2 (carried gap, unchanged — P9.08 §7):** the approval right is
  bound to qualification by RULE only; no code reads a qualification
  register (the enforcement wiring is a deployment composition item). R-9's
  status is therefore procedural until implemented, and V-1..V-4's run
  evidence must include the register check explicitly.
- **F-P10.15-3 (carried gap, unchanged — P9.26 §4 S-4):** scope expiry is
  procedural; no code enforces the control-scope document's expiry/review
  date. An expired scope holding only by human re-check is the recorded
  risk behind V-1's expiry test.
- **F-P10.15-4 (verification asymmetry):** the schema-level gate-ordering
  refusals (P10.13 T-4) prove the DATABASE defence-in-depth, not the
  application gate; the application-level bypass evidence comes from the
  P10.03/P10.04 suites and the V-1..V-4 candidate runs. Neither layer is
  run; neither substitutes for the other.
- **F-P10.15-5 (no candidate):** every V-cell depends on a candidate build
  (P10.18) and a deployed configuration that do not exist. The bypass
  verification is blocked, not passed.

## 13. Non-claims

No command was approved, issued, or acknowledged anywhere; no site was
promoted; no operator was qualified; no configuration was deployed,
inspected, or signed; no bypass attempt was executed; no automation envelope
exists or is enabled; no plugin was granted anything (the never-granted list
is the kernel's existing refusal discipline, cited — not exercised here).
Nothing here establishes operator-approved readiness, certification, safety
qualification, or field-write authority; the release ladder's v1.0.0 row is
a target, the 0.x line is simulation-only, and the entire evidence class is
simulation/lab-grade (AGENTS.md rules 2–5; ENTRY_NOTE boundaries 1–3).

Status: **NOT QUALIFIED (recorded)** — the qualification cannot "run" by
being written; the signatures, the candidate, the deployment configuration,
and the executions are the human gates and have not happened and are not
scheduled.
