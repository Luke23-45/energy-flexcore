<!-- P9.19 — Command-lifecycle dry run in shadow mode (decision record). DRY-RUN TEMPLATE ONLY: every run/result cell EMPTY, status UNRUN. No site exists (P9.01 freeze UNSIGNED), no shadow operation is running (P9.17 UNCHECKED), no command was ever proposed, gated, approved, or held in any pilot composition, and no ISSUED/ACKNOWLEDGED/TRACKING/COMPLETED transition was ever observed anywhere in a pilot context. The dry run itself is the human gate (ENTRY_NOTE.md boundary 6). -->
# P9.19 — Command-Lifecycle Dry Run in Shadow Mode (Template — every result cell EMPTY, UNRUN)

Date: 2026-11-14. Subtask: P9.19 (`ledger_phase_9.md` §P9.19). Owner
role: Platform engineering (with the pilot approver). Report trace:
§23 Stage 9; §14 (command lifecycle); §21.4 (read-only/shadow).
Inputs per the ledger: P9.03 grid model (TEMPLATE — no model version
exists), P9.14 lab results (UNRUN), P9.17 shadow operation (UNCHECKED),
P9.08 role matrix (DESIGN record — no person is assigned to any role,
`08_identity_access_approval.md` §1). None of the inputs is satisfied;
this record exists so the human gate has the complete, code-cited
procedure before any of them is satisfied.

## 1. Honest status — the plain statement

**No dry run has been executed, and none can be executed.** There is no
pilot site, no shadow deployment, no grid model version, no pilot
approver, and no pilot operator: the people and systems this dry run
needs do not exist. Every result cell in §8 is deliberately EMPTY
(`(EMPTY — UNRUN)`). The "expected behavior" statements in §4–§6 are
derived FROM READING the named code on 2026-11-14 (each citation names
the file and the mechanism verified to exist by reading) — they are
design statements about what the gates DO, not evidence that any pilot
command has ever traversed them. No lifecycle transition was performed,
no audit extract was captured, and no claim of lifecycle correctness is
made anywhere in this record.

## 2. What a dry run establishes — and what it does not

**A shadow dry run establishes exactly one thing:** that the command
state machine, as deployed in the pilot composition, transitions each
candidate command through the §14.1 gates and terminal states with each
gate distinct, each rejection named, each transition journalled with
actor/decision/evidence, and no command reaching ISSUED on any field
path. It is an evidence-generating rehearsal against real site
conditions (live telemetry, the frozen grid model) instead of fixtures.

**It does NOT establish:**

- **Field behaviour.** Shadow mode computes and compares proposed
  actions without issuing commands (§21.4). A command that passes every
  gate in shadow has never touched a device; correctness in shadow is
  correctness of the decision path only.
- **Delivery behaviour.** The ISSUED-onward transitions (acknowledgement,
  tracking, completion) are exercised, where at all, ONLY against the
  SIMULATOR transport (`dispatch_simulator.rs` —
  `SIMULATION_ONLY_STATEMENT`: "the dispatch adapter's transport target
  is the SIMULATOR ONLY …; no device write path exists in this code").
  An ACKNOWLEDGED command was accepted at the application layer by a
  simulator and was NOT delivered to any physical device.
- **Approval quality.** Simulated approval by the pilot approver role
  demonstrates that the authorization GATE enforces its conditions — it
  does not certify any human's judgement. Judgement is P9.21's subject.
- **Any safety, protection, or certification property** (AGENTS.md §3):
  the lifecycle is a supervisory control-plane state machine; FlexCore
  performs no primary frequency response, synthetic inertia, relay
  protection, inverter inner-loop control, or sub-cycle control (§14.5).

## 3. Design basis — the actual gate code (read 2026-11-14, cited not re-designed)

| Surface | File | What it provides the dry run |
|---|---|---|
| Frozen-edge oracle | `crates/flexcore-domain/src/command_lifecycle.rs` (P4.10) | The ONLY guard of record: an edge not in the frozen §14.1 27-edge table is impossible (`try_transition` → `NotPermitted`); 14 states; terminality derived from the frozen table |
| Creation + validation gate | `crates/flexcore-application/src/command_gates.rs` (P4.11) | Command created FROM AN APPROVED SCHEDULE only (SUPERSEDED/unknown revision rejected AT CREATION); validation re-checks schema, freshness (P3.13 staleness), grid model ACTIVE (P4.03), asset existence/capability/quantities, ramp, command policy; every rejection names its failing check |
| Feasibility gate | same file (P4.12) | VALIDATED → FEASIBLE \| REJECTED via the injected §12.5 runner; feasibility evidence is a SEPARATE reference from validation evidence (`feasibility:attempt/<attempt_id>:model/<model_id>:hash/<content_hash>`), bound to THE MODEL HASH; `CommandGates::ensure_command_issueable` is the pre-ISSUED check (§17.5 invariant 3) |
| Authorization gate | same file (P4.13) | `record_command_approval` is the ONLY producer of APPROVED; requires (a) an allow decision at the recorded scope, (b) approval scope covering the command's asset, (c) approver distinct from proposer where separation of duties is configured (§17.4), (d) expiry in the future. NO auto-approve path exists |
| Expiry / cancellation / supersede | `crates/flexcore-application/src/command_expiry.rs` (P4.15) | Expiry = validity-window SWEEP (D-P4.15-1) moving APPROVED/ISSUED/ACKNOWLEDGED/TRACKING → EXPIRED, never the other ten states; cancellation APPROVED→CANCELLED and ISSUED→CANCELLED only, actor + reason MANDATORY; supersede VALIDATED/FEASIBLE/APPROVED/ISSUED/ACKNOWLEDGED/TRACKING → SUPERSEDED with bidirectional links; all timestamps from the injected `LifecycleClock` |
| Simulation dispatch adapter | `crates/flexcore-application/src/dispatch_simulator.rs` (P4.16) | ISSUED requires a SIGNED, SEQUENCED command + `ensure_command_issueable` + ISSUE re-validation; ISSUED → ACKNOWLEDGED on application-layer ACCEPTANCE only; NACK/timeout → FAILED; the tracking evaluation order (deviation → FAILED; `min_satisfied_samples` within tolerance → COMPLETED; timeout → FAILED); `AdapterTargetCandidate::NonSimulator` REFUSED (D-P4.16-6) |
| Fallback engine | `crates/flexcore-application/src/fallback_policy.rs` (P4.17/P4.18) | FAILED/EXPIRED → FALLBACK on frozen edges 26–27 where the policy requires it; covered by `20_fallback_drills.md`, cited here only where a dry-run command enters FALLBACK |
| Edge verification (context) | `crates/edge-agent/src/command_verify.rs` (P6.15) | The four-check verifier a shadow command WOULD meet if the pilot composition had an enabled command path; in the dry run the path stays disabled — its rejection vocabulary is the expectation for P9.31, not exercised here |

The gate distinction, verbatim from `command_gates.rs` (Control status
section): "Validation, feasibility, and authorization are distinct gates
(§14.1); network feasibility is not operator authorization; an
acknowledgement is not physical delivery." And verbatim from the report
§14.1: "Validation, feasibility, and authorization are distinct. Network
feasibility is not operator authorization. An acknowledgement confirms
receipt/acceptance, not physical delivery."

**The hold-before-ISSUED mechanics, stated honestly.** The frozen table
has NO "held" state. The ledger's step-1 hold ("held before ISSUED, with
the hold recorded") is realized procedurally in the dry run: the shadow
composition never invokes the dispatch adapter, the dry-run record
records the decision NOT to invoke it per command, and acceptance
criterion (c) is verified by inspecting the audit trail for the ABSENCE
of any ISSUED transition on any dry-run command ID. A dry-run
composition where a command reaches ISSUED has failed the dry run —
that is the finding, not a recording artifact.

## 4. The lifecycle exercise — expected behavior per state (cited)

Band A — shadow composition, the pilot approver and operator roles
exercising the REAL gates (no dispatch adapter invocation):

| # | Transition exercised | Expected behavior (cited source) | Result |
|---|---|---|---|
| A-1 | (creation) approved schedule → PROPOSED | Created only from an ACTIVE schedule revision; a SUPERSEDED or unknown revision is rejected AT CREATION (P4.11 creation-source gate); the §14.3 authorization-reference placeholder `authz:pending:command/<id>` is recorded (it is never a decision) | (EMPTY — UNRUN) |
| A-2 | PROPOSED → VALIDATED | Schema (§14.3 validators), freshness, ACTIVE model, asset checks, ramp, policy checks ALL pass; journal row with validation evidence | (EMPTY — UNRUN) |
| A-3 | VALIDATED → FEASIBLE | §12.5 runner verdict pass; feasibility evidence reference binds the ACTIVE model hash and is a DISTINCT journal row/field from validation evidence (P4.11/P4.12 anti-conflation) | (EMPTY — UNRUN) |
| A-4 | FEASIBLE → APPROVED | `record_command_approval`: allow decision at recorded scope; scope covers the asset; approver ≠ proposer; expiry in future; `AuthorizationDecisionRecord` carries actor, role, scope, limits, expiry, decision time, and the EXACT validation + feasibility evidence IDs covered | (EMPTY — UNRUN) |
| A-5 | APPROVED → (held before ISSUED) | No dispatch-adapter invocation in shadow; the hold is the recorded non-invocation; NO ISSUED transition exists for the command ID (acceptance criterion c) | (EMPTY — UNRUN) |
| A-6 | APPROVED → EXPIRED | The validity-window sweep moves APPROVED → EXPIRED when `valid-until` passes before issue, recording the expiry time AND the milestone not reached (`ExpiryMilestone::BeforeIssue`, frozen edge 10) | (EMPTY — UNRUN) |
| A-7 | APPROVED → CANCELLED | Explicit operator/policy act; actor + reason MANDATORY (the domain engine refuses an empty reason); distinct journal row, distinct event from supersede | (EMPTY — UNRUN) |
| A-8 | VALIDATED or FEASIBLE → SUPERSEDED | A newer approved replacement moves the command to SUPERSEDED; supersedes/superseded-by links bidirectional; record RETAINED (append-only, nothing deleted) | (EMPTY — UNRUN) |

Band B — SIMULATION COMPOSITION ONLY, clearly labelled as such
(`SIMULATION_ONLY_STATEMENT`; every command here targets the simulator
and nothing else; if the pilot composition cannot express Band B, the
cells record that fact, not a proxy figure):

| # | Transition exercised | Expected behavior (cited source) | Result |
|---|---|---|---|
| B-1 | APPROVED → ISSUED | Only for a SIGNED, SEQUENCED command; the simulation keyring's `sim-sig:` reference is a non-cryptographic, clearly-labelled simulation identity, NOT production PKI; `ensure_command_issueable` (active-model-hash feasibility evidence) + ISSUE re-validation run first | (EMPTY — UNRUN) |
| B-2 | ISSUED → ACKNOWLEDGED | Only on application-layer ACCEPTANCE from the simulator; acceptance is NEVER physical delivery | (EMPTY — UNRUN) |
| B-3 | ISSUED → FAILED | A NACK or transport timeout/failure moves ISSUED → FAILED with the frozen-table trigger | (EMPTY — UNRUN) |
| B-4 | ACKNOWLEDGED → TRACKING | Observed telemetry starts the measured-vs-requested comparison | (EMPTY — UNRUN) |
| B-5 | TRACKING → COMPLETED | At least `min_satisfied_samples` within-tolerance observations (criterion = deployment configuration, never hard-coded) | (EMPTY — UNRUN) |
| B-6 | TRACKING → FAILED | Any observed sample outside tolerance → deviation FAILED (a recorded deviation fails the command even if the asset recovers); or tracking timeout elapsed → FAILED | (EMPTY — UNRUN) |
| B-7 | TRACKING/APPROVED/ISSUED → EXPIRED | Validity ends before the next milestone; sweep + domain guard; milestone recorded | (EMPTY — UNRUN) |
| B-8 | ACKNOWLEDGED/TRACKING → SUPERSEDED | Replacement during tracking via the P4.15 service; tested end-to-end in the crate's own tests; the pilot run re-demonstrates it in ITS composition | (EMPTY — UNRUN) |
| B-9 | FAILED/EXPIRED → FALLBACK (where policy requires) | Frozen edges 26–27 through the domain engine only; evidence reference `fallback-policy:<name>`; drill depth is P9.20's | (EMPTY — UNRUN) |

## 5. Gate-separation cases (acceptance criterion b: each gate fails independently)

| # | Candidate | Expected outcome (cited) | Result |
|---|---|---|---|
| G-1 | Malformed candidate (schema violation) | PROPOSED → REJECTED at VALIDATION; no feasibility check runs; `DispatchRejected` emitted; the fact of record is the journal row | (EMPTY — UNRUN) |
| G-2 | Valid schema, infeasible against the frozen model | VALIDATED → REJECTED at FEASIBILITY with the limit/verdict reason; `FeasibilityCheckCompleted` emitted for the completed check (P4.30); validation evidence alone would NOT have rejected it | (EMPTY — UNRUN) |
| G-3 | Valid + feasible, unauthorized (approver role lacks `command.approve` at the command's scope, or approver == proposer, or expiry in the past) | FEASIBLE → REJECTED at AUTHORIZATION; the P4.13 gate refuses with its named condition; validation and feasibility evidence remain on record and did not authorize anything | (EMPTY — UNRUN) |
| G-4 | Candidate whose schedule revision turns SUPERSEDED mid-chain | Rejected — at creation if known, and at feasibility via the revision check; the seam and the persistence-layer gate AGREE by construction | (EMPTY — UNRUN) |
| G-5 | Stale candidate (freshness gate) | PROPOSED → REJECTED at VALIDATION (P3.13 staleness); never "validated with a note" | (EMPTY — UNRUN) |

## 6. Per-command invariant checklist (§14.3; acceptance criterion d)

Every dry-run command carries, verified from the command record and
journal (cells filled per command at the run):

| Invariant (§14.3) | Present? | Evidence ref |
|---|---|---|
| unique command ID | (EMPTY — UNRUN) | (EMPTY) |
| tenant, portfolio, site, asset IDs | (EMPTY — UNRUN) | (EMPTY) |
| source subsystem and plugin/model version | (EMPTY — UNRUN) | (EMPTY) |
| source schedule ID and schedule revision | (EMPTY — UNRUN) | (EMPTY) |
| reason/action and actor or service identity | (EMPTY — UNRUN) | (EMPTY) |
| target P/Q or supported control parameter | (EMPTY — UNRUN) | (EMPTY) |
| units and sign convention (canonical MW/MVAr/MWh; P > 0 = export) | (EMPTY — UNRUN) | (EMPTY) |
| valid-from / valid-until timestamps | (EMPTY — UNRUN) | (EMPTY) |
| duration and ramp requirements | (EMPTY — UNRUN) | (EMPTY) |
| monotonic sequence/version per asset or control channel | (EMPTY — UNRUN) | (EMPTY) |
| idempotency key | (EMPTY — UNRUN) | (EMPTY) |
| grid model and telemetry snapshot references | (EMPTY — UNRUN) | (EMPTY) |
| validation evidence reference | (EMPTY — UNRUN) | (EMPTY) |
| authorization decision reference (approved scope, limits, expiry) | (EMPTY — UNRUN) | (EMPTY) |
| signature or authenticated transport identity | (EMPTY — UNRUN) | (EMPTY) |
| supersedes/superseded-by reference (where applicable) | (EMPTY — UNRUN) | (EMPTY) |

## 7. Audit-trail checks (ledger step 5)

| Check | Expected (cited) | Result |
|---|---|---|
| Each transition journalled | One immutable journal row per transition: from, to, actor, timestamp, reason, evidence reference (`command_expiry.rs`: the journal row is the evidence of record; the frozen §15.3 payloads carry identity + time only — actor/reason travel in the row) | (EMPTY — UNRUN) |
| Events paired with state changes | State change first, then the transactional-outbox write, paired through the store receipt (`command_gates.rs`, `command_expiry.rs`) | (EMPTY — UNRUN) |
| No `DispatchValidated`/`DispatchFeasible` event exists | Deliberately absent from the frozen §15.3 vocabulary (recorded gap, P0.14) — the VALIDATED/FEASIBLE facts live in the journal | (EMPTY — UNRUN) |
| Timestamps all from the controlled clock | Every timestamp from the injected `LifecycleClock`; timestamp-regression guard enforced | (EMPTY — UNRUN) |
| No ISSUED for any dry-run command ID | The ledger's verification method — inspect the audit trail per command ID | (EMPTY — UNRUN) |

## 8. RUN RECORD — ALL RESULT CELLS EMPTY. NOTHING HAS BEEN EXERCISED.

### 8.0 Run conditions (filled by the human gate from the run)

| Run | Date | Site / composition (shadow vs simulation — Band A/B) | Grid-model version (P9.03) | Control-authority mode (§21.4) | Software versions | Proposer (Operator) | Approver (TenantAdmin at site/asset — per `08_...md` §3.1; NOT the proposer) | Witnesses |
|---|---|---|---|---|---|---|---|---|
| EMPTY | (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |

### 8.1 Per-case results

| Case | §4/§5/§6/§7 ref | Observed outcome | Pass/fail | Evidence extract filed at |
|---|---|---|---|---|
| (EMPTY — UNRUN) | (EMPTY) | (EMPTY) | (EMPTY) | (EMPTY) |

## 9. Reviewer-verification procedure (human)

1. Confirm §1: no dry run was executed; every §8 cell is
   `(EMPTY — UNRUN)`; no transition, audit extract, or figure is
   fabricated anywhere in this record.
2. Confirm every "expected behavior" claim in §4–§7 traces to the cited
   file and mechanism (spot-check at least `record_command_approval`'s
   four conditions and the sweep's state set in
   `command_gates.rs`/`command_expiry.rs`).
3. Confirm the two bands are honestly separated: Band A never invokes
   the dispatch adapter; Band B is simulation-transport-only and
   labelled; no cell describes a device write.
4. Confirm the hold-before-ISSUED statement (§3): no "held" state was
   invented; acceptance (c) is an audit-trail absence check.
5. Confirm the approver column of §8.0 can never be filled with the
   proposer (P9.08 separation of duties; the P4.13 gate refuses it in
   code regardless).

## 10. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to this record):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) preconditions (ledger): P9.03 model frozen; P9.14 lab passed;
#     P9.17 shadow operating; P9.08 roles ASSIGNED (people exist) —
#     none of which is true today. Do not start before all four hold.

# (c) THE RUN (human gate): execute §4 Band A in the shadow composition,
#     then §4 Band B in the simulation composition (or record that the
#     composition cannot express it); execute §5 G-1..G-5; fill the §6
#     invariant checklist per command; capture the §7 audit extracts;
#     fill §8.0–§8.1 FROM THE RUN OUTPUT ONLY.
# (d) THE LEDGER'S VERIFICATION METHOD: inspect the audit trail for the
#     dry-run command IDs; confirm no ISSUED state exists for any of
#     them; file the extracts as the P9.19 evidence named in
#     CHECKLIST.md ("Command lifecycle dry-run passed").
```

## 11. Open risks carried to P9.27

| # | Risk | Basis |
|---|---|---|
| R-1 | A gate that passes everything in shadow because the validator is pointed at a stale model — the ledger's named failure mode; mitigated by the ACTIVE-model check in code but the model-freshness AT THE SITE is a run-time condition | §3; ledger risks |
| R-2 | Approval simulated by the same person who proposed — a P9.08 separation-of-duties violation; the code gate refuses it, but the human process must assign two distinct people before the run | §8.0 approver column; P9.08 §4.1 |
| R-3 | An expired command treated as issuable — mitigated by the sweep + the pre-ISSUED re-validation in code; the drill proves it in the pilot composition | §4 A-6; §4 B-1 |
| R-4 | Band B may be unexpressible in the pilot composition (no simulator transport wired at the pilot) — the cells then record that fact; ISSUED-onward coverage then rests on the P9.15 HIL/lab evidence | §4 Band B preamble |
| R-5 | Shadow lifecycle correctness is not field delivery evidence — the residual risk that shadow-passed commands behave differently when a real command path is enabled at v1.0.0 promotion (P9.26/P9.17 gate) | §2 |
| R-6 | No dry run, site, model, or approver exists — every ledger acceptance criterion is UNMET and not claimed | §1, §8 |

## 12. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| The dry run itself (any cell of §8) | **NOT RUN — banned in this loop; human-gate input; all result cells EMPTY** |

Status: **WRITTEN-UNRUN** — a template cannot be "run"; the shadow
execution is the human gate and has not happened and is not scheduled.
No command was created; no pilot exists and none is claimed.
