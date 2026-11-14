<!-- P9.17 — Field acceptance test execution, shadow first (decision record). EXECUTION TEMPLATE ONLY: the run record is EMPTY — no site, no dates, no personnel, no per-case result exists. No pilot site exists (P9.01 freeze UNSIGNED); commissioning has never completed anywhere (P9.13 UNSCHEDULED); the shadow review has never been held; no promotion has ever been requested. EXECUTION IS THE HUMAN GATE (ENTRY_NOTE.md boundary 6). Shadow-first ordering is structural: no operator-approved consideration before shadow acceptance. -->
# P9.17 — Field Acceptance Test Execution, Shadow First (EMPTY Execution Record Template)

Date: 2026-11-14. Subtask: P9.17 (`ledger_phase_9.md` §P9.17).
Owner role: Integration engineering (with the customer operator).
Report trace: §23 Stage 9; §19.2 (item 9 shadow-mode evaluation; item 10
ordering); §21.4 (mode promotion requires §22.3 gates plus site-specific
acceptance).
Inputs: P9.13 commissioning complete per site (**NOT COMPLETE — no site
exists**), P9.14 lab report (**NOT EXECUTED**), P9.15 HIL report
(**UNRUN**), P9.16 FAT plan (**DRAFT — customer review UNSIGNED**),
P9.06 D6 closure (**OPEN — not signed**), frozen versions per P9.16
(**NOT RECORDED**). **None of the inputs exists.**

## 1. Honest status — the plain statement

**No field acceptance test has been executed anywhere.** No site has
completed commissioning, no site is in shadow operation, no shadow case
has run, no shadow review has been held, no promotion has been
requested, and no operator-approved acceptance case exists. There is no
site (`01_pilot_scope_freeze.md` §3.I: UNSIGNED), no customer, no
dates, no personnel roster, and no result of any kind. This record
delivers what an agent can deliver under ENTRY_NOTE.md boundary 6:

- the **shadow-first execution procedure** (§3) with the ordering gate
  (§2) fixed BEFORE anything runs;
- the **run record as an EMPTY template** (§5) — one per site; site,
  dates, personnel, frozen versions, and per-case results ALL EMPTY;
- the **shadow-review record template** (§6) — the customer gate that
  must close before any promotion consideration;
- the **witness requirements** (§4);
- the **boundaries** (§7): what a shadow FAT at one site would and would
  not establish — it establishes nothing about other sites or scales;
- the **reviewer-verification procedure** (§8) and exact filling
  commands (§9).

Filling any §5/§6 cell from anything other than the actual execution is
fabrication (AGENTS.md rule 1). This record never executes, approves a
promotion, or grants field write authority.

## 2. The ordering gate — shadow first, verbatim (fixed BEFORE any run)

### 2.1 The release-ladder position (verbatim, AGENTS.md §4 table rows)

| Release | Meaning | Control status |
|---|---|---|
| v0.4.0 | First site integration (OpenEMS), protocol adapters | Read-only / test-lab by default |
| v1.0.0 | Pilot-ready | Operator-approved control by default |
| v1.x / v2.x | Advanced coordination / grid intelligence | Per-feature qualification |

And the standing rule verbatim (ENTRY_NOTE.md boundary 3): **"The pilot
is operator-approved control AT MOST (v1.0.0 ladder). Bounded
automation is post-1.0. Nothing in Phase 9 grants field write
authority."** Bounded automation (§21.4: "Post-1.0, per deployment,
site, and asset class after qualification") is OUT OF SCOPE for this
record and for the pilot, per the signed P9.01 freeze exclusion.

### 2.2 The §21.4 mode rows (verbatim)

| Mode | Authority | Writes to devices | Default use |
|---|---|---|---|
| Read-only / shadow | None; computes and compares proposed actions against measurements | No | Early pilots and commissioning |
| Operator-approved | Each command requires an authorized human approval | Yes, after approval | v1.0.0 default |

And §21.4's promotion rule verbatim: **"Promotion to a higher-authority
mode requires the release gates in §22.3 plus site-specific
acceptance."** Control-authority mode is a deployment and per-site
setting, not something a plugin can select.

### 2.3 The ordering rules (structural — a violation is a recorded finding, never repaired)

| # | Rule |
|---|---|
| O-1 | Every site starts FAT execution in **Read-only / shadow** (§21.4). There is no configuration in which a site starts FAT in operator-approved mode |
| O-2 | ALL shadow cases (the P9.16 §6.3–§6.6 catalogue) run and are dispositioned (pass, signed waiver, or recorded gap acceptance) BEFORE any promotion request exists |
| O-3 | The **shadow review** (§6) — the customer reviewing shadow results, including tracking error and any rejected commands WITH their reasons — happens and is signed BEFORE any mode promotion is REQUESTED |
| O-4 | Promotion to operator-approved mode is REQUESTED per P9.26 (the control-scope document with the §22.3 gates recorded) and granted by the customer/site decision — the request never grants itself |
| O-5 | Operator-approved acceptance cases run ONLY after shadow acceptance AND the granted promotion, under direct operator supervision |
| O-6 | ANY failed operator-approved case ⇒ the site reverts to shadow mode and the reversion is RECORDED; no further commands are issued at that site until a re-accepted promotion. A failed command is never treated as a "test artifact" with the site left in operator-approved mode (the ledger's named failure mode) |
| O-7 | Skipping shadow mode to save time is the ledger's named failure mode; O-1–O-6 make it a protocol violation rather than an option |

## 3. Execution procedure (per the ledger steps; every step's record cell EMPTY)

| Step | Procedure | Record cell (§5) |
|---|---|---|
| E-1 | Confirm the site completed commissioning (P9.13 C8.3 shadow-entry sign-off SIGNED) and is in read-only/shadow mode; confirm the §19.2(10) reviews passed (P9.41/P9.14/P9.07 + signed D6) and the P9.16 plan is customer-signed (§9) with its §6.2 thresholds registered | Entry-gate block (all EMPTY) |
| E-2 | Confirm the frozen version set (P9.16 §4) is recorded BEFORE the first case; any version change during execution follows the P9.16 §4 invalidation and re-run rules | Version-freeze block (EMPTY) |
| E-3 | Execute the P9.16 cases in shadow mode: FlexCore computes and compares proposed actions against measurements WITHOUT issuing commands (§21.4). Record every result with the frozen versions, timestamps, and the operator present, the same day, from actual output | Per-case shadow result table (EMPTY) |
| E-4 | File failures per the P9.16 §7 failure process (retry only after a recorded fix or a customer-signed waiver — never re-run until it passes) | Failures-and-dispositions table (EMPTY) |
| E-5 | Hold the shadow review: the customer reviews shadow results — tracking error and every rejected command with its reason included — and signs BEFORE any promotion request | Shadow-review record (§6; EMPTY/UNSIGNED) |
| E-6 | Only after shadow acceptance, request promotion to operator-approved mode per P9.26; execute the operator-approved acceptance cases (approved command issue, acknowledgement, tracking, completion) under direct operator supervision | Promotion-request block + operator-approved result table (EMPTY) |
| E-7 | On any operator-approved case failure: revert the site to shadow mode, RECORD the reversion, stop issuing commands at that site (O-6) | Reversion table (EMPTY — exists only if a failure is recorded) |

## 4. Witness requirements

| Witness | Required for | Rule |
|---|---|---|
| Customer operator | Every case (shadow and operator-approved) | Present and named in the per-case row; the signature evidences presence, not Neryva-only acceptance (the ledger's named failure mode: "acceptance witnessed only by Neryva") |
| Customer site contact | Every case at their site | Co-signs the per-case row |
| Authority witness (where required) | The cases the applicable authority's engagement record (P9.12) designates, for THIS site | If no authority requires witnessing, the witness cell records "none required" WITH its basis from the P9.12 record — never a silent omission |
| Independent reviewer | The post-execution review (§8) | Did not execute any case; inspects the record for version consistency and the shadow-before-promotion order; confirms the customer operator's signature on the shadow review (the ledger's verification method) |

All witness cells are **UNSIGNED / EMPTY** — no roster exists.

## 5. RUN RECORD — ALL CELLS EMPTY. NO SITE EXISTS. NOTHING HAS RUN.

**One record per site.** Filling any cell from anything other than the
actual execution is fabrication (AGENTS.md rule 1).

### 5.1 Site and entry gate

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Site ID (signed P9.01 freeze) | |
| Commissioning record reference (P9.13 C8.3, signed) | |
| D6 closure reference (P9.06, signed for this deployment) | |
| §19.2(10) review references (P9.41 / P9.14 / P9.07) | |
| P9.16 plan reference + customer-review signature ref | |
| P9.16 §6.2 threshold-registration amendment ref | |
| P9.40 map version filed with the P9.16 plan (TR-6) | |

### 5.2 Frozen versions and run conditions

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Control-plane build/version | |
| Edge-agent build (git SHA) | |
| Adapter versions / ACL mapping table version | |
| Site EMS/OpenEMS version | |
| Grid-model version (P9.03) | |
| Device firmware identities (P9.01) | |
| Deployed configuration manifest (P9.13 C8.1) + digest | |
| Control-authority mode in force per phase (§21.4) | |
| Run dates (start / per-phase / end) — UTC, actual | (ALL EMPTY — no date is invented) |
| Executors / operators present (names, per case) | (EMPTY) |
| Authority witness (or "none required" + P9.12 basis) | (EMPTY) |

### 5.3 Per-case shadow results (one row per P9.16 case; every cell EMPTY)

| Case ID (P9.16) | Mode (shadow — invariant) | Result (PASS / FAIL / waived / INVALIDATED-version-change / UNMAPPED-GAP-OPEN) | Observed behaviour (from the actual output) | Versions in force | Operator present | Evidence reference | Date (actual) |
|---|---|---|---|---|---|---|---|
| FAT-T1 | (EMPTY — UNRUN) | | | | | | |
| FAT-T2 | (EMPTY — UNRUN) | | | | | | |
| FAT-T3 | (EMPTY — UNRUN) | | | | | | |
| FAT-T4 | (EMPTY — UNRUN) | | | | | | |
| FAT-S1 | (EMPTY — UNRUN) | | | | | | |
| FAT-S2 | (EMPTY — UNRUN) | | | | | | |
| FAT-S3 | (EMPTY — UNRUN) | | | | | | |
| FAT-S4 | (EMPTY — UNRUN) | | | | | | |
| FAT-S5 | (EMPTY — UNRUN) | | | | | | |
| FAT-F1 | (EMPTY — UNRUN) | | | | | | |
| FAT-F2 | (EMPTY — UNRUN) | | | | | | |
| FAT-F3 | (EMPTY — UNRUN) | | | | | | |
| FAT-F4 | (EMPTY — UNRUN) | | | | | | |
| FAT-F5 | (EMPTY — UNRUN) | | | | | | |
| FAT-F6 | (EMPTY — UNRUN) | | | | | | |
| FAT-F7 | (EMPTY — UNRUN) | | | | | | |
| FAT-F8 | (EMPTY — UNRUN) | | | | | | |
| FAT-R1 | (EMPTY — UNRUN) | | | | | | |
| FAT-D1 | (EMPTY — UNRUN) | | | | | | |
| FAT-O1 | (EMPTY — UNRUN) | | | | | | |
| FAT-A1 | (EMPTY — UNRUN) | | | | | | |

Per-case semantics (fixed in advance, the P9.16 §6.1 rule): PASS =
observed behaviour matches the case's expected behaviour, on the
recorded versions, with the named evidence filed, in the recorded mode.
A case marked passed on versions differing from the frozen set is
INVALID. A waived case carries its FP-3 customer-signed waiver
reference. A case whose result rests on a HIL pass rather than this
run is NOT A RESULT — P9.40 rule R-2 (a HIL pass justifies execution,
never the field result).

### 5.4 Failures and dispositions

| Failed case | Disposition (fixed-and-re-run per P9.16 §4 re-run rules / customer-signed waiver FP-3 / blocks the site) | Evidence | Status |
|---|---|---|---|
| (none — exists only when a failure is recorded from actual output) | | | (EMPTY — UNRUN) |

### 5.5 Promotion and operator-approved phase

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Shadow-review record reference (§6, signed) | |
| P9.26 promotion request reference + §22.3 gate evidence | |
| Promotion GRANTED BY (customer/site decision, name, date) | |
| Operator-approved case results (issue / acknowledgement / tracking / completion — one row each) | |
| Direct operator supervision record (who, per case) | |
| Any reversion to shadow (O-6): case, reason, reversion record | |

## 6. SHADOW-REVIEW RECORD — EMPTY TEMPLATE (the customer gate before any promotion request)

| Field | Value (EMPTY — fill at the review) |
|---|---|
| Site ID | |
| Review date (actual, UTC) | |
| Shadow results reviewed (the §5.3 table, including tracking error and every rejected command WITH its reason — the ledger step 4 requirement, verbatim scope) | |
| Tracking error summary as reviewed (per §19.4 dispatch metrics; EMPTY — no measurement exists) | |
| Rejected commands reviewed (each with its named reason) | |
| Customer operator verdict (accept / accept-with-conditions / reject) | |
| Conditions, if any | |
| Customer operator signature / date | **UNSIGNED** |
| Customer site contact signature / date | **UNSIGNED** |
| Neryva product owner acknowledgement / date | **UNSIGNED** |

**O-3 is structural: no promotion request (§5.5) may exist before this
record is signed. The reviewer (§8) checks the order explicitly.**

## 7. What FAT execution would and would not establish

**A completed, signed shadow FAT at one site would establish ONLY:**
that THIS site's deployment, at the frozen versions recorded in §5.2,
behaved as the P9.16 catalogue expects for the executed cases, on the
recorded dates, in read-only/shadow mode, with the recorded evidence —
i.e. §19.2 item 9's shadow-mode evaluation satisfied for THAT site,
and the customer's shadow acceptance (§6) recorded for THAT site.

**It would NOT establish** (none of these may be written about it):

1. **Anything about other sites.** A shadow FAT at one site establishes
   nothing about any other site — every other site requires its own
   commissioning (P9.13), its own FAT execution against its own frozen
   versions, and its own shadow review. One site's pass is never
   carried to another site as evidence.
2. **Anything about scale.** No extrapolation beyond the tested site
   and asset set (§19.3 verbatim discipline: "Do not claim that the
   first distribution feeder tests establish capability for a
   10,000-bus utility network"). More sites, more assets, or higher
   telemetry rates are untested territory.
3. **Any operator-approved behaviour.** A shadow result is computed-
   and-compared evidence (§21.4); it says nothing about how commands
   behave when actually issued. That evidence exists only from the
   operator-approved cases after shadow acceptance and the granted
   promotion (O-4/O-5) — and only for that site.
4. **The field RESULT from upstream evidence.** A HIL pass (P9.15), a
   lab pass (P9.14), or a simulation pass justifies the FAT case's
   EXECUTION and pre-declares its expected behaviour — it never
   justifies, substitutes for, or predicts the field result (P9.40
   rule R-2). A FAT case is never marked passed because its HIL row
   passed.
5. **Safety, certification, or protection-grade properties** (AGENTS.md
   §3 rules 2–3): the FAT evidences supervisory integration behaviour
   in shadow; site-local protection stays independent (§17.5 invariant
   10); IEEE 1547.1 conformance remains a separate responsibility
   (§11.3).
6. **Field write authority beyond the recorded scope.** Operator-
   approved mode is the pilot's CEILING (v1.0.0 ladder, §2.1); bounded
   automation is post-1.0 and excluded; a FAT signature never widens
   the P9.26 control-scope document.
7. **A finite test set is not a guarantee** (the X-4 discipline): the
   executed cases cover the catalogue's enumerated behaviours for the
   recorded period; they are not a proof about all future conditions.

A FAILING case is recorded AS THE OUTCOME (§5.4) — it does not
invalidate the platform; it invalidates the acceptance claim for that
case until dispositioned per the P9.16 §7 process.

## 8. Reviewer-verification procedure (the ledger's verification method)

1. Confirm §1's plain statement is unchanged: no site, date, personnel,
   or result exists anywhere in the record; every §5/§6 cell is EMPTY /
   UNRUN / UNSIGNED.
2. Confirm §2's ordering rules are intact and quoted verbatim (the
   ladder rows; the §21.4 rows; the §21.4 promotion rule).
3. For a filled future record, the ledger's checks, mechanically:
   (a) inspect the execution record for VERSION CONSISTENCY — every
   result row carries the frozen versions and any version change is
   marked with the P9.16 §4 invalidation labels; (b) inspect for the
   SHADOW-BEFORE-PROMOTION ORDER — the §6 shadow-review signature
   predates the §5.5 promotion request, which predates every
   operator-approved case row; (c) confirm the CUSTOMER OPERATOR's
   signature on the shadow review; (d) confirm every failed
   operator-approved case has a recorded reversion (O-6) and no case
   was dispositioned as a "test artifact".
4. Confirm no result rests on upstream evidence in place of the run
   (P9.40 R-2) and no coverage-unmapped case was executed as if
   covered.
5. Confirm §7's boundaries are intact in any summary written about the
   filled record (no cross-site, cross-scale, or shadow→operator-
   approved inference).
6. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the template is written; no execution exists.

## 9. Filling commands (for the human executor)

```sh
# (a) Documented-tools gate (before and after filling):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill order: §5.1 entry gate (commissioning signed, D6 signed,
#     reviews passed, P9.16 signed + thresholds registered, P9.40 map
#     version filed) → §5.2 versions frozen → E-3 shadow cases IN THE
#     P9.16 CATALOGUE ORDER, filling §5.3 from ACTUAL output the same
#     day → E-4 dispositions (§5.4) → E-5 shadow review (§6) signed →
#     E-6 P9.26 promotion request + §22.3 evidence → granted promotion →
#     operator-approved rows (§5.5) under direct supervision → E-7
#     reversion row ONLY if a failure occurs (O-6).

# (c) Per-case evidence helper — hash each evidence artifact as filed:
#     sha256sum <evidence-file>   # record the digest with the reference

# (d) Honesty grep — no fabricated figure may exist before execution:
grep -n "UNRUN\|UNSIGNED\|EMPTY" ledger/phase-09-pilot-qualification/17_field_acceptance_execution.md | head -40
#     The committed template must show §5/§6 EMPTY labels verbatim.

# (e) Cross-references: P9.38 consumes the recorded shadow period;
#     P9.39 consumes the shadow-operation load context; P9.26 consumes
#     the shadow-review verdict; P9.28 consumes the whole record.
```

No execution happened in this writing loop (banned; ENTRY_NOTE
boundary 6); every record cell stays EMPTY until people at a real site
fill it.

## 10. Open risks carried to P9.27 (this subtask's contribution)

| # | Risk | Why it is open |
|---|---|---|
| R-1 | **No site, no commissioning, no shadow data** — the entire record is unfillable until P9.13 completes for a real site with signed D6 | §1, §5.1 |
| R-2 | **Skipping shadow mode to save time** (the ledger's named failure mode): O-1–O-7 are structural, but enforcement at a real pilot is human | §2.3 |
| R-3 | **A failed operator-approved command treated as a "test artifact"** with the site left in operator-approved mode (the ledger's named failure mode): O-6 mandates the recorded reversion; the §8 check (d) audits it | §2.3 O-6, §5.5 |
| R-4 | **Acceptance evidence recorded without the versions that produced it** (the ledger's named failure mode): the §5.2 freeze block and §5.3 version column are mandatory per row | §5.2, §5.3 |
| R-5 | **Shadow review perfunctory** — a signature without the tracking-error and rejected-command review the ledger step 4 requires would satisfy the letter of O-3 while voiding its substance; the reviewer checks the review CONTENT, not just the signature | §6 |
| R-6 | **HIL pass over-read as a field result** (P9.40 R-2): the strongest pressure on this record once upstream passes exist | §5.3 semantics; §7 item 4 |
| R-7 | **Cross-site/cross-scale over-read**: a successful first-site FAT tempts fleet claims; §7 items 1–2 forbid it and the P9.27 register should carry the boundary | §7 |
| R-8 | **Promotion pressure before P9.26 exists**: the promotion request cites the P9.26 control-scope document and §22.3 evidence; if P9.26 is not signed, O-4 blocks — recorded here as an open dependency | §2.3 O-4 |

## 11. Verification status (honest)

| Item | Status |
|---|---|
| `python tools/verify_docs.py` | RUN in the writing loop (output quoted in the completion report) |
| `python tools/validate_agent.py` | RUN in the writing loop (output quoted in the completion report) |
| `python -I tools/check_dependency_rule.py` | RUN in the writing loop (no dependency edge added) |
| The FAT execution (any cell of §5/§6) | **NOT RUN — banned in this loop; human-gate input; no site exists** |
| Shadow review | **NEVER HELD — UNSIGNED** |
| Promotion request | **NEVER REQUESTED — O-4 not reachable today** |
| Any site, date, personnel, or result | **NONE EXISTS — nothing fabricated** |

Status: **WRITTEN-UNRUN** — FAT execution is people at a real site; it
has not happened, is not scheduled, and cannot be executed by an agent.
Every record cell is EMPTY and stays EMPTY until filled from actual
execution output.
