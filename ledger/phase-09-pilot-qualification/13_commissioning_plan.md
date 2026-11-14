<!-- P9.13 — Site commissioning plan (decision record). PLAN TEMPLATE ONLY: every execution/date/signature cell EMPTY or UNSCHEDULED. No site exists (P9.01 freeze UNSIGNED); no commissioning step has been scheduled, performed, or witnessed. Hardware-dependent rows are PENDING PILOT HARDWARE SPEC (P6.34). The shadow-entry gate below is pre-printed; it has never been evaluated because no site has ever reached it. -->
# P9.13 — Site Commissioning Plan (Plan Template — Execution Cells EMPTY)

Date: 2026-11-14. Subtask: P9.13 (`ledger_phase_9.md` §P9.13).
Owner role: Integration engineering (with the customer site contact).
Report trace: §23 Stage 9; §19.2(10) (HIL/commissioning tests run only
after simulation, integration, and cyber reviews pass); §21.2 (a central
outage must not disable site-local protective controls or force an unsafe
device state); §21.4 (read-only/shadow entry mode).
Inputs: P9.02 reconciled registry (TEMPLATE — no site data), P9.03 frozen
grid model (TEMPLATE — no model version), P9.04 integration design
(TEMPLATE — no adapter inventory), P9.05 site network design (TEMPLATE —
no zone membership, no allowlist rules), P9.09 secrets/certificates
record (no PKI exists in the 0.x line), P9.33 enrollment-ceremony record
(ceremony CODE WRITTEN-UNRUN; the ceremony EXECUTION is itself a human
gate), P9.06 D6 closure (OPEN — not signed), vendor commissioning
documentation (**does not exist — no vendor is named**), site access and
safety procedures from the customer (**do not exist — no site exists**).

## 1. Honest status — the plain statement

**No commissioning has been scheduled, planned for a date, or performed,
and none is claimed.** No pilot site exists (`01_pilot_scope_freeze.md`
§3.I: UNSIGNED), so there is no site to walk, no hardware to deliver, no
network to bring up, and no customer site contact to review anything.
This record delivers what an agent can deliver under ENTRY_NOTE boundary
1: the **phase-by-phase commissioning PLAN** (§4) that a human executor
follows to take a real site from "installed" to "accepted for shadow
operation" — every step with a named owner, an expected result, a
pass/fail criterion, and a named evidence artifact — with **every
execution cell EMPTY (UNSCHEDULED)**, every date cell EMPTY, and every
sign-off UNSIGNED. Hardware-dependent rows are labelled **PENDING PILOT
HARDWARE SPEC** (P6.34: no edge platform, board, or boot path is named
anywhere in-repo). Acceptance criterion (d) — customer site contact
review — is **NOT MET and NOT CLAIMED** (§8).

Nothing here configures, connects, installs, or energizes anything. A
commissioning plan is paper; commissioning is people, tools, and a
physical site — all three of which are absent.

## 2. What commissioning is — and what it is not (scope ceiling)

- Commissioning here means ONLY: verifying that the installed FlexCore
  edge components, their network path, and their identity material are
  correctly installed, correctly connected, and behave as designed in
  read-only/shadow posture — ending in a site sign-off that permits
  entering shadow operation (§21.4 read-only/shadow ceiling).
- **It does not grant field write authority.** The command path stays
  DISABLED through the entire commissioning sequence (Phase C6 tests the
  disabled state as a negative). Promotion beyond shadow is the P9.16–
  P9.17/P9.26 chain with its own §22.3 gates — deferred, not part of this
  plan (§7).
- **It is not a safety or certification activity** (AGENTS.md §3 rules
  2–3). Site-local protection, switching, and electrical safety remain
  with the customer's local certified controllers and procedures (§17.5
  invariant 10). Every commissioning step that touches energized
  equipment is performed BY THE CUSTOMER under THEIR safety rules —
  FlexCore personnel and tooling observe and record only (§5).
- **It is not HIL evidence.** HIL (P9.15) and the lab integration test
  (P9.14) are separate, earlier gates on the §19.1/§19.2 ladder; §19.2(10)
  puts commissioning tests AFTER simulation, integration, and cyber
  reviews pass — the pre-gate table in Phase C0 enforces that ordering.

## 3. Phase C0 — Commissioning entry gate (preconditions; all currently UNMET)

Commissioning for a site may be SCHEDULED only when every row below is
closed for that site. At writing, every row is **UNMET**. The gate
exists so the ledger's §19.2(10) ordering ("only after simulation,
integration, and cyber reviews pass") is checked before mobilization,
not after.

| # | Precondition | Source | Status at writing |
|---|---|---|---|
| C0.1 | Pilot scope frozen and signed, naming THIS site and its assets | P9.01 `01_pilot_scope_freeze.md` §3.I | **UNMET — UNSIGNED, no site** |
| C0.2 | Registry reconciled for the site's assets, zero unresolved discrepancies | P9.02 `02_site_asset_reconciliation.md` | **UNMET** |
| C0.3 | Grid model version frozen and validated for the site | P9.03 `03_network_model_build.md` | **UNMET** |
| C0.4 | Integration design FILLED (adapters, versions, profiles) for the site | P9.04 `04_customer_integration_design.md` | **UNMET — template only** |
| C0.5 | Site network design approved by the customer network/OT owner | P9.05 `05_site_network_design.md` §12 | **UNMET — NOT REVIEWED** |
| C0.6 | D6 CLOSED for the deployment (signed per-deployment record) | P9.06 `06_ot_it_boundary_d6.md` §6 | **UNMET — D6 OPEN** |
| C0.7 | Secrets/certificates inventory ready: no secret in source control; edge credential path resolvable | P9.09 | **UNMET — record not written in this wave** |
| C0.8 | Simulation, integration, and cyber reviews passed (§19.2(10)) | P9.41 regression re-run; P9.14 lab report; P9.07 assessment with no open must-fix | **UNMET — all UNRUN/unwritten** |
| C0.9 | Lab integration test passed for the pilot configuration (P9.14) | P9.14 | **UNMET — NOT EXECUTED** |
| C0.10 | Edge hardware platform NAMED (x86-64 / ARM64 / container / customer-managed), build profile chosen, secure-boot determination recorded | P6.34 `ledger/phase-06-edge-protocol/34_secure_boot_position.md` §2–§3 | **PENDING PILOT HARDWARE SPEC** |

Rule: a commissioning visit mobilized with any C0 row unmet is a protocol
violation; the plan's own record (§6 run template) captures the C0
verdicts first, and any UNMET row aborts scheduling (Phase C0 exit =
all rows MET).

## 4. The commissioning sequence (phases C1–C8; execution cells EMPTY)

One sequence PER SITE (site IDs come from the signed P9.01 freeze; none
exist — the site column stays `<SITE-ID>` throughout). Each step lists
procedure, expected result, pass/fail criterion, owner, and the evidence
artifact to file. Every "Scheduled / Performed / Result" cell is EMPTY —
UNSCHEDULED. The command path is DISABLED in every phase; no step in this
plan issues or forwards a command.

### Phase C1 — Delivery and physical installation check

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C1.1 | Receive edge hardware; verify model/serial against the named platform (C0.10) and the P9.01 asset/install list | Delivered unit matches the named platform profile | Match, recorded serial-for-serial; any substitution stops the phase | Integration engineering (with customer site contact) | Delivery note + photo of nameplate (signed) | **UNSCHEDULED — EMPTY** |
| C1.2 | Physically install the edge host in the zone the P9.05 design assigns (Zone 3 / DMZ position per the design's §5 pattern) | Host mounted per P9.05 §3 zone membership | Install location matches the FILLED P9.05 diagram exactly; deviation = fail, design change first | Integration engineering + customer site contact | Install checklist, photo, annotated zone diagram | **UNSCHEDULED — EMPTY** |
| C1.3 | Physical interfaces only as designed: power feed, network drops to the two allowlisted endpoints (control plane; site EMS). No other connection is made | Cabling matches the P9.05 §4 flow table (F1, F2 only from the edge) | Every cable maps to a documented flow row; an undocumented cable is a FAIL and a P9.05/P9.06 re-review trigger | Integration engineering (customer electrician energizes) | Cable schedule vs P9.05 §4 table | **UNSCHEDULED — EMPTY** |

Hardware-dependent rows in C1: the edge platform model, its mounting/
power requirements, and any secure-boot enablement at first boot are
**PENDING PILOT HARDWARE SPEC** (P6.34 — `edge-x86_64-systemd`,
`edge-aarch64-systemd`, `edge-oci`, customer-managed variants all
UNDETERMINED). C1.1 cannot name hardware and C1.2/C1.3 cannot be
finalized until the human hardware selection closes C0.10.

### Phase C2 — Power and network bring-up (per P9.05)

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C2.1 | Customer energizes the edge host; boot to the installed OS state | Host reaches the OS; systemd reaches `time-sync.target` before the agent starts | Boot log shows `time-sync.target` reached; the agent started only after it (§17.4 clock rule, `clock_quality.rs` design) | Integration engineering (customer performs energization) | Boot log extract | **UNSCHEDULED — EMPTY** |
| C2.2 | Bring up the outbound network leg to the control plane per the P9.05 §6 allowlist (F1) | Outbound TCP to the control-plane endpoint succeeds; NO inbound path exists | (i) F1 allowlist rule verified effective; (ii) an external port-scan/probe of the edge shows NO listening surface — matching the structural guarantee (`crates/edge-agent/tests/no_inbound_listener.rs`: no `TcpListener`/`UdpSocket`/bind/listen/accept in the shipped code); (iii) no other port reachable | Integration engineering + customer network owner | Firewall rule ID + probe log (both filed) | **UNSCHEDULED — EMPTY** |
| C2.3 | Bring up the edge→EMS leg (F2) per the P9.05 allowlist | Edge reaches the site EMS endpoint on the designed hop | Reachability per the design; **the OpenEMS REST hop protection gap (`http://` today, P6.26 §9 item 5) is either closed or explicitly accepted in the signed D6 record** — D6 OPEN = fail | Integration engineering | Reachability log; D6 record reference | **UNSCHEDULED — EMPTY** |
| C2.4 | Verify the standing denies (P9.05 §6): edge host CANNOT reach databases, NATS, plugin surfaces; Zone-5 devices cannot reach FlexCore surfaces | All deny rules effective | Each pre-printed deny verified by an attempted (logged, refused) connection | Customer network/OT owner | Deny-rule test log | **UNSCHEDULED — EMPTY** |
| C2.5 | Record the buffer bound and outage-duration assumption as deployed configuration | Buffer bound matches the P9.05 §7 reviewed value | Deployed bound == signed design value; mismatch = fail, configuration change first | Integration engineering | Configuration extract vs design row | **UNSCHEDULED — EMPTY** |

### Phase C3 — Edge-agent software installation and enrollment ceremony (per P9.33)

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C3.1 | Install the signed edge bundle per the P6.20 build/package shape; verify the bundle signature before activation | Signature verification passes BEFORE the bundle activates | Verification must run and pass pre-activation; a bundle whose verification cannot run (no PKI — P6.34 carried status "no PKI is implemented") FAILS this step and blocks commissioning until the P9.34 control is verified | Integration engineering | Install log with verification output | **UNSCHEDULED — EMPTY** |
| C3.2 | Execute the enrollment ceremony per `33_edge_enrollment.md` §"filling commands": platform identity authority issues a single-use, ≤15-minute token (out-of-band delivery); ceremony runs `Unenrolled → … → Active` through the outbound mTLS enrollment flow | Ceremony reaches `Active` exactly once, on the intended (tenant, site) binding | (i) token single-use and within lifetime; (ii) NO self-approval; (iii) identity register (`FileIdentityEventRegister`) shows the full transition history; (iv) binding matches the P9.01/P9.26 site scope | Integration engineering + platform identity authority (human approver) | Ceremony register extract + approval record | **UNSCHEDULED — EMPTY** |
| C3.3 | Negative enrollment case (ledger P9.33 step 4 requires it): attempt token reuse / expired token / wrong-site binding | Each refused with the named ceremony errors; nothing partially applied | Every negative attempt refused; no state change in the register beyond the refusal event | Integration engineering | Refusal log | **UNSCHEDULED — EMPTY** |
| C3.4 | Compromise re-enrollment walk-through (paper drill at the bench): `Revoke → ResetForReEnrollment` shown to work on the installed edge with the revoked identity unable to authorize | Re-enrollment path demonstrable | Revoked identity refused as authorizer (P9.33 rule) | Integration engineering | Drill log | **UNSCHEDULED — EMPTY** |

### Phase C4 — Identity and certificate check (mTLS posture)

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C4.1 | Confirm the uplink session is mTLS both directions; scheme `wss://`/`https://`; TLS floor as pinned by the signed D6 record (DEFAULT TLS 1.3; `tls12-floor` only per signed record) | Session negotiates per D6-pinned policy | Observed negotiated version/cipher matches the D6 record; a `ws://`/`http://` uplink is an automatic FAIL | Integration engineering | TLS handshake capture | **UNSCHEDULED — EMPTY** |
| C4.2 | Confirm the edge verifies the CENTRAL certificate against the root CA pinned from the credential record (fail-closed on empty/unparsable store) | Central identity verified edge-side | Handshake with a non-pinned central cert is refused (negative test) | Integration engineering | Negative-test log | **UNSCHEDULED — EMPTY** |
| C4.3 | Confirm credential lifetime/rotation posture per P9.09 (no secret in config; redacted logs) | Config carries references only; logs show no secret material | Log/config inspection finds no secret; redaction verified | Integration engineering + security | Redaction inspection record | **UNSCHEDULED — EMPTY** |
| C4.4 | Record the edge clock quality state as seen at commissioning (`clock_quality.rs`) and the site's sync source/tolerance per the D6 record | Clock `Synchronized` with the D6-declared source; non-`Synchronized` fails the expiry check CLOSED | Clock quality state recorded; the fail-closed behaviour is CONFIRMED, not assumed | Integration engineering | Clock-quality observation | **UNSCHEDULED — EMPTY** (measured drift values are P9.18's deliverable, not this plan's) |

### Phase C5 — Telemetry flow check (read path, end-to-end)

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C5.1 | With the edge in read-only posture, confirm telemetry flows device → EMS → edge adapter → outbound mTLS session → control plane → operator console, for EVERY adapter named by the FILLED P9.04 design | Each named adapter delivers samples end-to-end | Every P9.04 adapter row delivers; any adapter absent from the flow = fail (no adapter is "assumed working") | Integration engineering (customer site contact observes) | Per-adapter flow trace with timestamps | **UNSCHEDULED — EMPTY** |
| C5.2 | At each boundary, verify canonical units (MW/MVAr/MVA/MWh; SOC fraction [0,1]) and sign conventions — including the reactive-power sign DECLARED per device profile (never assumed) | Values arrive in canonical units with declared signs | Each checked point matches the P9.04 §mapping row; any undeclared or mismatched sign = fail | Integration engineering (with power systems) | Units/sign comparison table | **UNSCHEDULED — EMPTY** |
| C5.3 | Verify telemetry quality states (§10.4: exactly one of GOOD/SUSPECT/BAD/MISSING/ESTIMATED/STALE per value) and the freshness/stale mapping (`openems.rs` OPENEMS_STALE_CODE → canonical STALE) | Quality states map per the frozen ACL table; degraded never upgraded to GOOD | Each mapped code matches the ACL table; an upgrade observed anywhere = fail | Integration engineering | Quality-state trace | **UNSCHEDULED — EMPTY** |
| C5.4 | Link-loss resilience check: drop the F1 uplink for a bounded interval; restore | Edge buffers locally (bounded store-and-forward, `buffer_file.rs`); no data loss beyond the audited drop-of-oldest policy; no new commands while disconnected (the command channel IS the down session) | Buffered records re-uploaded in order, at-least-once; drop events audited (`audit_buffer.rs`); buffer never exceeded its bound | Integration engineering | Buffer/audit journal extract | **UNSCHEDULED — EMPTY** |

### Phase C6 — Command-path check (DISABLED — negative test)

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C6.1 | From the control plane, attempt to issue a command to the commissioned site in the DEFAULT (post-commissioning) configuration | Command is refused BEFORE issue — the §14 command lifecycle does not reach ISSUED; the refusal is audited | (i) zero commands reach the edge or any adapter; (ii) the refusal appears in the platform audit trail; (iii) no adapter received anything (`command_forward.rs` requires a `VerifiedCommand`; enablement is derived read-only in `command_path.rs` — there is no setter to flip) | Integration engineering + security witness | Command-attempt audit extract | **UNSCHEDULED — EMPTY** |
| C6.2 | Negative verification set at the edge boundary (uses the P9.31 vocabulary WITHOUT live-site injection; run in the LAB first per P9.14, and at commissioning only as a bench-level confirmation): forged signature / expired / replayed / out-of-sequence / conflicting-idempotency | Each refused with the named rejection code; sequence checkpoint unchanged after each | Every case refused (`pilot_rejection_proof.rs` codes: `signature`/`expiry`/`duplicate`/`sequence`/`idempotency_conflict`); zero adapter reach | Integration engineering | Rejection log + checkpoint state extract | **UNSCHEDULED — EMPTY** |
| C6.3 | Confirm the deployed enablement configuration matches the P9.26 control-scope document (when it exists) and the signed D6 conditions | Command path disabled; enablement requires the P6.14 four-condition gate incl. the site's signed D6 record | Config inspection matches the control-scope document; any divergence = fail | Security | Configuration-vs-control-scope diff | **UNSCHEDULED — EMPTY** |

### Phase C7 — Fallback and degraded-mode check

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C7.1 | Central-platform outage drill (bounded): stop the control plane; observe the site | Site continues LOCAL operation; site-local protection unaffected (§21.2, §17.5 invariant 10); edge buffers and refuses new commands | Site EMS/local controllers behave independently throughout; no unsafe device state attributable to FlexCore | Integration engineering + customer site operator | Drill log + operator confirmation | **UNSCHEDULED — EMPTY** (the full per-policy drill register is P9.20's deliverable) |
| C7.2 | Edge-agent restart drill: restart the agent | Sequence checkpoint and audit buffer persist (`FileSequenceCheckpoint`, `FileAuditBuffer`); no replay window regression | Restarted agent resumes with persisted state; no accepted command re-processed | Integration engineering | Restart log + state extract | **UNSCHEDULED — EMPTY** |
| C7.3 | Signed-update/rollback check (re-verifies P9.06 UP3 at commissioning, as the D6 record requires): staged update + failed-health rollback on the bench profile | Rollback to the previous verified bundle occurs on failed health; downgrade only via explicit operator-signed action | Rollback demonstrable on the INSTALLED platform (the P6.34/P6.20 path has never executed — this is its first physical verification) | Integration engineering + platform engineering | Update/rollback log | **UNSCHEDULED — EMPTY** |

### Phase C8 — Configuration freeze, review, and shadow-entry sign-off

| Step | Procedure | Expected result | Pass/fail criterion | Owner | Evidence artifact | Scheduled/Performed/Result |
|---|---|---|---|---|---|---|
| C8.1 | Freeze the site's deployed configuration (versions, profiles, buffer bound, allowlist, TLS floor, enablement state) into the commissioning record | Frozen set matches every prior phase's evidence | Any drift between recorded evidence and deployed state = fail before freeze | Integration engineering | Frozen configuration manifest (hashed) | **UNSCHEDULED — EMPTY** |
| C8.2 | Commissioning walkthrough with the customer site contact: every phase's evidence reviewed against this plan | Every step has a recorded PASS (or a dispositioned fail, §6) and its named artifact | Zero steps without a criterion outcome; zero unsigned artifacts | Integration engineering + customer site contact | Walkthrough minutes | **UNSCHEDULED — EMPTY** |
| C8.3 | Shadow-entry evaluation: confirm EVERY commissioning step passed AND D6 is closed for the site (P9.06 signed record) | Entry condition met | The site enters shadow operation ONLY on: all C1–C7 passes + C8.2 walkthrough complete + SIGNED D6 record for this deployment + P9.14 lab report on record. Any single miss = no entry | Integration engineering + customer site contact (signatories below) | Signed shadow-entry authorization | **UNSCHEDULED — EMPTY** |

## 5. Site safety rules for commissioning (structure; site values EMPTY)

These rules are the plan's pre-printed floor. The customer's own site
safety procedures override and supplement them; the site-specific cells
are EMPTY until the customer site contact fills them at plan review (§8).

| # | Rule | Site-specific fill (EMPTY) |
|---|---|---|
| S1 | **Who may touch equipment:** FlexCore personnel never energize, de-energize, open, or wire ANY energized component. All electrical work is performed by the customer's qualified personnel under the customer's procedures | `<EMPTY — named roles>` |
| S2 | **Lock-out / tag-out (LOTO):** where the site requires LOTO, every step that could contact energized equipment follows the customer's LOTO procedure; FlexCore's own steps assume the equipment is in its NORMAL operating state and touch nothing beyond the edge host and its cabling | `<EMPTY — site LOTO procedure reference>` |
| S3 | **Stop condition for any step:** any of — an unexpected physical state (energized conductor exposed, unlabelled cable, overheating); a telemetry value that would trip a site limit; any command observed reaching an adapter during commissioning; any refusal to fail closed. Hitting a stop condition halts commissioning; the site does NOT enter shadow operation on a halted sequence | `<EMPTY — site emergency contacts>` |
| S4 | **Scope of physical authority:** the commissioning team's authority is limited to the edge host, its mounts, and its two designed network legs. Anything else requires the customer's explicit, recorded authorization | `<EMPTY>` |
| S5 | **Evidence preservation:** an aborted or failed commissioning preserves all logs and audit journals before any remediation (the P9.23 evidence rule, applied here) | `<EMPTY — evidence location>` |

## 6. Failure handling, rollback, and the run record — EMPTY TEMPLATE

**Per-step failure:** a failed step stops its phase. The executor records
the failure, preserves the evidence (S5), and escalates to the rollback
decision. The options and decision owner, per ledger step 4:

| Outcome | Who decides | What is restored |
|---|---|---|
| Retry after a bounded fix | Integration engineering, with the customer site contact informed | The step re-runs from its start; prior passes are NOT invalidated unless the fix touched an earlier phase's scope (then that phase re-runs) |
| Defer the site | Commissioning lead + customer site contact (recorded decision) | The site halts at its last passed phase; NO shadow entry; remaining sites unaffected; deferral recorded in the run record |
| Abort the site | Commissioning lead + customer site contact + Neryva product ownership (recorded decision) | The edge is decommissioned per the decommission path (`decommission_p6_32.rs` discipline); identity revoked (`Revoked` state, P9.33); network legs removed per the P9.05 allowlist; abort recorded |

**Run record template — EVERY CELL EMPTY (UNSCHEDULED).** One per site.
Filling any cell from anything other than the actual commissioning
activity is fabrication (AGENTS.md rule 1).

| Field | Value (EMPTY — fill at execution) |
|---|---|
| Site ID (signed P9.01 freeze) | |
| C0 gate verdicts (C0.1–C0.10, per row) | |
| Hardware platform (closes the P6.34 PENDING cells) | |
| Per-phase results (C1.1–C8.3: pass / fail / dispositioned — one line each) | |
| Failed steps + disposition (fixed-and-re-run / site deferred / site aborted) | |
| Safety events (S3 stop conditions hit; none is an acceptable default only if none occurred) | |
| Frozen configuration manifest reference (C8.1) | |
| Shadow-entry authorization reference (C8.3) | |
| Dates: mobilization / per-phase / walkthrough / shadow entry | (ALL EMPTY — no date is invented) |
| Customer site contact (name, signature, date) | **UNSIGNED** |
| Neryva commissioning lead (name, signature, date) | **UNSIGNED** |
| Security witness for C6 (name, signature, date) | **UNSIGNED** |

## 7. What commissioning establishes and what it defers

**Established by a completed, signed commissioning (per site):** the
installed edge components, network path, identity material, telemetry
read path, disabled command path, and fallback behaviour matched this
plan's criteria, on the recorded date, on the recorded hardware and
software versions — and the site is therefore ACCEPTED FOR SHADOW
OPERATION (read-only, §21.4), with the frozen configuration manifest as
the baseline the rest of Phase 9 tests against.

**Explicitly deferred by this plan (never implied by a commissioning
signature):** operator-approved control (P9.16/P9.17 acceptance + P9.26
control-scope + §22.3 gates); bounded automation (excluded in writing by
the P9.01 freeze §3.F); HIL evidence (P9.15 — separate rig, separate
record); performance/load figures at pilot scale (P9.39); telemetry
freshness/clock measurements at operating precision (P9.18); the
command-lifecycle dry run in shadow (P9.19); field acceptance (P9.17).
Commissioning proves installation and read-only posture; it proves
nothing about control behaviour beyond the demonstrated disabled state.

## 8. Customer site contact review — NOT REVIEWED

Acceptance criterion (d). The plan is not "ready" until this row is
filled and signed; until then the plan is a reviewed-baseline DRAFT.

| Field | Value |
|---|---|
| Reviewer (customer site contact, named) | `<EMPTY>` |
| Review basis (walkthrough vs vendor commissioning documentation — ledger verification method) | `<EMPTY>` |
| Outcome (approved / approved-with-conditions / changes required) | **NOT RECORDED** |
| Conditions / required changes | `<EMPTY>` |
| Signature / date | **UNSIGNED / `<EMPTY>`** |

Vendor commissioning documentation check (ledger input): the plan's C1–C3
rows must be walked against the ACTUAL vendor documentation for the named
platform (C0.10). No vendor document is cited here because none exists
in-repo; the reviewer confirms the walkthrough happened and records it
above.

## 9. Reviewer verification procedure

1. Confirm §1's plain statement is unchanged; no scheduled date, performed
   step, measurement, or signature exists anywhere in the record.
2. Confirm every "Scheduled/Performed/Result" cell, every §6 run-record
   cell, and every §8 review cell is EMPTY / UNSCHEDULED / UNSIGNED /
   NOT RECORDED.
3. Confirm every C1–C8 step has ALL of: procedure, expected result,
   pass/fail criterion, owner, named evidence artifact (ledger acceptance
   criterion (a)) — no step may lack a criterion (the ledger's named
   failure mode: "steps written without pass/fail criteria, so everything
   passes").
4. Confirm the §4 C8.3 entry condition requires the SIGNED D6 record
   verbatim (criterion (c)), and §5's safety rules + stop conditions are
   written (criterion (b)).
5. Confirm every hardware-dependent row carries the PENDING PILOT
   HARDWARE SPEC label and traces to `ledger/phase-06-edge-protocol/
   34_secure_boot_position.md` — no "secure boot: yes" or platform claim
   anywhere.
6. Confirm the negative-command posture: no step in this plan issues or
   forwards a command; C6 tests the disabled state as a negative.
7. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — a plan is written; nothing was commissioned.

## 10. Filling commands (for the human executor)

```sh
# (a) Verify the documented-tools gate before and after filling:
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) Fill order (dependencies): close C0 preconditions in ledger order
#     (P9.01 → P9.02 → P9.03 → P9.04 → P9.05 → P9.06 signed D6 → P9.09
#     → P9.07 → P9.14 lab report → C0.10 hardware spec). THEN:
#     - schedule mobilization; fill §6 run-record date cells as they ACTUALLY occur;
#     - execute phases C1–C8 in order; fill each step's result cell from the
#       named evidence artifact at the time of the step — never afterwards from memory;
#     - file every artifact at the location recorded in the run record;
#     - fill §5 site cells WITH the customer site contact at the plan review (§8);
#     - C8.3 shadow-entry signature is the LAST cell filled.

# (c) Config-freeze helper (C8.1) — hash the frozen manifest so drift is detectable:
#     sha256sum <frozen-manifest-file>   # record the digest in the run record

# (d) Post-commissioning cross-references: P9.17 (field acceptance, shadow
#     first) consumes the frozen manifest; P9.18 (telemetry/clock) consumes
#     the C4.4/C5 observations; P9.20 (fallback drills) consumes C7.
```

No commissioning activity is executable in this writing loop (loop bans;
ENTRY_NOTE boundary 1): every execution cell stays EMPTY until people at
a real site fill it.

## 11. Open risks carried to P9.27 (this subtask's contribution)

| Risk | Basis |
|---|---|
| No site, no customer, no hardware — commissioning is UNSCHEDULED and un-schedulable until the P9.01 freeze is signed and C0.10 names hardware | §1, §3 C0; ENTRY_NOTE boundary 1 |
| D6 OPEN — the shadow-entry condition cannot be met; a site entering shadow without signed D6 closure is the ledger's named failure mode and this plan blocks it structurally (C8.3) | §4 C8.3; P9.06 §10 |
| Edge hardware platform undetermined (P6.34): secure-boot determination, build profile, and the C1 install procedure are PENDING PILOT HARDWARE SPEC; commissioning cannot be finalized for any platform until named | §4 C1; P6.34 §2–§3 |
| Signed-update path DESIGN ONLY (no PKI in 0.x; P6.34 carried status): C3.1 fails closed until P9.34 verification exists — commissioning is blocked, not waived, by this gap | §4 C3.1 |
| OpenEMS REST hop cleartext (`http://`) inside the OT boundary: C2.3 fails unless closed or explicitly accepted in the signed D6 record | §4 C2.3; P6.26 §9 item 5 |
| Vendor commissioning documentation absent: C1–C3 walkthrough criteria may not match the actual platform's vendor procedure — the §8 review must reconcile them | §8; ledger inputs |
| Safety rules assumed from another site (the ledger's named failure mode): §5 prevents it structurally but the site fill is a human step not yet possible | §5 |
| A failed commissioning step dispositioned as "accepted" without a recorded decision owner — the §6 table binds the disposition to named decision owners | §6 |
| Buffer bound set from unmeasured assumptions (P9.05 §7): C2.5 can only verify design==deployed, not that the bound is correct for a real outage — P9.18/P9.39 measurement risk carried | §4 C2.5; P9.05 §7 |

Status: **WRITTEN-UNRUN** — a commissioning plan cannot be "run" by an
agent; the only executable steps (mobilization, physical work, ceremony
execution, walkthrough, sign-off) are human gates that have not happened
and are not scheduled. No site exists, no hardware is named, no date is
invented, and no sign-off is claimed.
