# P9.36 — Live incident drill: forged-command containment — the PLAN + EMPTY run record

Date: 2026-11-14. Subtask: P9.36 (`ledger_phase_9.md` §P9.36). Owner role:
Security (with product/operations and the customer). Report trace: §23
Stage 9; §17.3 (incident response and customer notification process);
§17.4 (forged/replayed command); §18.2 (the edge validates every command).
Inputs: P9.23 procedure + tabletop record (**procedure WRITTEN 2026-11-14 by `23_incident_response.md` §3 — the tabletop-signed half remains OPEN and this drill stays UNSCHEDULABLE; see §1,
open dependency**); P9.31 attack-test record
(`31_edge_rejection_proof.md` — the attack-case vocabulary); P9.17 shadow
operation (NOT SCHEDULED). Precedents: the drill-plan pattern of
`ledger/phase-07-plugin-sdk/30_failure_drill.md` and
`ledger/phase-08-advanced-coordination/24_failure_drill.md` (expected
behaviours cited from code by file+symbol; the run record EMPTY; the
witness a human gate).

---

## 0. Run-status honesty — READ FIRST

**The drill has NOT been run and CANNOT be run by an agent.** ENTRY_NOTE.md
boundary 1 and boundary 6 make every live drill a HUMAN/ORGANIZATION gate
input: "For every such subtask the deliverable is the PLAN / RECORD /
TEMPLATE with EMPTY execution cells, exact filling commands, and honest
UNRUN/UNSIGNED/NOT-SCHEDULED labels. Never fabricate a site, a customer, a
date, a measurement, or a sign-off." Accordingly:

- **Every execution cell in §5 is deliberately EMPTY (UNRUN — human gate
  input).** No detection time, no containment time, no notification time,
  no witness identity is claimed anywhere in this document.
- Nothing was executed in this wave: no cargo command, no worker process,
  no network injection, no site activity. The loop bans hold.
- The "expected behaviour" cells in §3 are DESIGN statements derived FROM
  READING the named code (each cell cites file + symbol verified to exist
  by reading on 2026-11-14) — they are NOT evidence the drill ran and NOT
  a claim the behaviour works until the human build/test pass and the live
  drill say so.
- **The inject delivery, the detection, the escalation, the customer
  notification, and the containment action are all performed by PEOPLE at
  the pilot, under the P9.23 procedure.** This document plans that
  execution; it never performs it.

## 1. Open dependencies (recorded, not hidden)

| Dependency | State at writing (2026-11-14) | Consequence |
|---|---|---|
| P9.23 incident-response + customer-notification procedure + tabletop record | **NOT YET WRITTEN** (P9.23 is a later-wave Phase 9 subtask; no record exists in this directory) | The drill CANNOT be scheduled until P9.23's procedure exists and its tabletop (its step 4, a forged/replayed-command tabletop with the customer) has run — the ledger itself names both as inputs. Until then this drill stays UNSCHEDULABLE, not merely UNRUN. |
| P9.31 attack-test record + human build/test pass | Record written (`31_edge_rejection_proof.md`); the 9-test matrix WRITTEN-UNRUN | The inject vocabulary (§2) is the recorded `PilotAttackCase` set; the drill pre-flight (§4 step 2) requires the human build/test pass to have run that matrix against the pilot edge build first — the ledger P9.31 acceptance criterion (c) is exactly that precondition. |
| P9.17 shadow operation at the pilot | NOT SCHEDULED (human gate) | The drill runs against a site IN shadow/read-only mode; no site can host it before P9.13–P9.17 complete. |
| P9.25 observability/alerting verification | NOT YET WRITTEN (later-wave subtask) | The ALERT-PATH leg of the containment chain (§3, leg 4) is exercised against whatever alerting P9.25 has verified; until then the alert leg is recorded as infrastructure-PENDING in the run template. |
| P9.07 security assessment | In production by the concurrent wave (not read, not cited for content) | The drill's incident classification must match P9.07's findings and P9.23's classes when both exist; this plan pre-declares the class (§2 step 0) and defers the mapping. |

**Scheduling rule (recorded):** P9.36 is executable only after ALL of:
P9.23 procedure + tabletop signed; P9.31 matrix run against the pilot edge
build; the site in shadow per P9.17; P9.25 alert path verified. Any run
before all four is a protocol violation, and its record cannot close this
subtask.

## 2. The drill scenario (the plan)

**Incident class (P9.23 step-1 vocabulary, pre-declared):** "suspected
command forgery/replay" — a forged-command attempt injected at the pilot
edge boundary, detected, contained, notified, and evidenced, with the
affected site reverted to shadow/local control (the ledger's containment
action, step 2).

**The inject (ledger step 1):** the P9.31 attack-case vocabulary, used as
the inject set. The CLOSED case set is
`crates/edge-agent/src/pilot_rejection_proof.rs` :: `PilotAttackCase`
(`forged_signature`, `replayed_captured`, `expired_window`,
`out_of_sequence`, `conflicting_idempotency_payload`) with the
per-case expected rejection codes
(`PilotAttackCase::expected_rejection_codes`: `signature` / `duplicate` /
`expiry` / `sequence` / `idempotency_conflict`). Extending the set is a
reviewed change to that enum — never an ad-hoc live payload invented for
the drill.

**Delivery channel (the live-drill difference from the code-level proof):**
the code-level P9.31 matrix proves the mechanics IN-PROCESS with injected
clocks and real gate receipts. The live drill delivers at least one
ForgedSignature inject through the pilot's REAL command-delivery path (the
response side of the outbound mTLS session per §3.3 — there is no inbound
listener to inject against: `crates/edge-agent/src/uplink_mtls.rs` and the
structural `crates/edge-agent/tests/no_inbound_listener.rs`), so the
detection/escalation/notification legs run against real operators, real
clocks, and real audit files. The replay/expiry/sequence/idempotency cases
may be exercised at the edge locally (the composition-root harness) or
delivered live; the drill record states per case which was used (§5
"Injection used" column).

**Steps (the executable spec):**

0. **Pre-flight (all human):** confirm the four §1 preconditions (recorded
   with evidence references); freeze the software/edge-build versions for
   the drill (the P9.16 rule: a version change during the run invalidates
   the affected cells); brief the customer contact and the on-call first
   responder; set the notification deadline clock (the P9.23-recorded
   deadline for the incident class — the deadline is READ from the P9.23
   procedure, never invented here).
1. **Evidence baseline (ledger step 3, BEFORE any inject):** export and
   anchor the edge audit chain per P9.32 (`crates/edge-agent/src/audit_proof.rs`
   :: `compute_chain_anchor`; the anchor is retained by the central
   platform). Record: the persisted sequence-checkpoint state
   (`FileSequenceCheckpoint::path`), the idempotency-binding journal state
   (`FileCommandIdBinding::len` / `records_written`), the audit-buffer
   counters (`FileAuditBuffer::appended_total` / `pending_len`), and the
   site's current control-authority mode (shadow — from the mode record).
2. **Inject:** deliver the chosen attack case with the injection instant
   recorded from the operator's log (never from memory — the P7.30
   precedent rule).
3. **Detection (ledger step 2, leg 1):** the first responder identifies the
   attempt from the audit/telemetry surface — the expected audit evidence
   is the named event lines of §3 (greppable, e.g. `EDGE_PILOT_ATTACK_REJECTED`,
   `EDGE_COMMAND_REJECTED`, `EDGE_COMMAND_ID_BINDING_CONFLICT`). Detection
   time = first audit/log line naming the condition MINUS the injection
   instant, both from the logs.
4. **Escalation + customer notification (ledger step 2, legs 2–3):** the
   first responder executes the P9.23 procedure AS WRITTEN — escalation
   path, customer contact, notification within the recorded deadline.
   Notification time vs deadline is recorded (§5). A deadline miss is a
   drill FAILURE (acceptance criterion c), never a note.
5. **Containment action (ledger step 2, leg 4):** revert the affected site
   to shadow or local control per the P9.23 immediate-containment step; the
   reversion is confirmed IN THE SITE'S MODE RECORD (the ledger's
   verification method) and audited. Reverting the WRONG site is the
   ledger's named failure mode — the record must name the site ID reverted
   and match the injected command's site binding
   (`DeviceBinding` tenant/site).
6. **Evidence preservation (ledger step 3):** BEFORE any remediation:
   export the post-inject audit chain and verify it against the §2-baseline
   anchor plus a NEW post-drill anchor (`recompute_chain_head` /
   `verify_against_anchor`); preserve the checkpoint state (unchanged —
   see §3), the binding-journal state, and the telemetry snapshots around
   the window. Confirm nothing was destroyed by the response (the ledger's
   criterion d): the record states which artifacts were preserved and by
   whom.
7. **After-action:** timings (detection-to-containment; notification vs
   deadline), corrections to the P9.23 procedure (criterion e), and the
   witness sign-off. The drill record's witness fields are filled ONLY by
   the human witness.

## 3. Expected behaviours — cited from the code (file + symbol, read 2026-11-14)

The containment chain has FIVE legs; each attack case maps to named code
and named audit evidence. Every cell below is a code-derived expectation,
UNRUN at the live drill.

| Leg | Expected behaviour (code-derived, UNRUN) | Cited from |
|---|---|---|
| 1. Gate admission/rejection | A forged command arriving at the edge hits the P6.14 gate first: in the default build an `ISSUED` command is REJECTED (`GateRefusal` codes; `CommandRefusal::AdapterNotAllowlisted`; the policy check via `SitePolicyEngine::admit_command`), and the rejection is audited `EDGE_COMMAND_REJECTED` with a `command-denial:<id>/<adapter>@<ts>/<code>` reference built by `CommandRejectionAuditRecord::audit_text`. A gate-admitted command then runs the P6.15 verifier. | `crates/edge-agent/src/command_path.rs` :: `CommandReceiver::receive`, `GateRefusal::code`, `CommandRefusal::code`, `CommandRejectionAuditRecord`; `crates/edge-agent/src/policy.rs` :: `SitePolicyEngine::admit_command`, `PolicyViolation::code` |
| 2. Verification rejection (per case) | `forged_signature` → `CommandVerificationRejection::Signature` (`check_signature`: unsigned / malformed reference / wrong signer key reference / payload-binding mismatch / digest mismatch — each a distinct reason string). `replayed_captured` → `Duplicate` (sequence == last on `sequence_channel(asset, "command")`; the `FileSequenceCheckpoint` survives restart, so a post-restart replay is still `duplicate`). `expired_window` → `Expiry` (window AND authorization window against the INJECTED clock; any clock quality other than `Synchronized` fails closed — `ClockQuality` seam). `out_of_sequence` → `Sequence` (checked LAST; a refused command never advances the window). `conflicting_idempotency_payload` → refused by the ADDITIVE `FileCommandIdBinding::bind` guard BEFORE the verifier, named `PilotRejectionProofError::ConflictingIdempotencyPayload`, audited `EDGE_COMMAND_ID_BINDING_CONFLICT`. | `crates/edge-agent/src/command_verify.rs` :: `CommandVerifier::verify`, `check_signature`, `check_scope`, `CommandVerificationRejection::code`, `FileSequenceCheckpoint`; `crates/edge-agent/src/pilot_rejection_proof.rs` :: `FileCommandIdBinding::bind`, `PilotRejectionProofError` |
| 3. Zero adapter reach | A rejected verification produces NO `VerifiedCommand`; the ONLY route from a gate-admitted command to an adapter is a `VerifiedCommand` through `CommandForwardingEngine::forward_verified` (its own separate allowlist + policy re-checks). A counting forwarder at the seam must show ZERO invocations across all injected cases. | `crates/edge-agent/src/pilot_rejection_proof.rs` (module header, the structural statement); `crates/edge-agent/src/command_forward.rs` :: `CommandForwardingEngine::forward_verified`, `ForwardingRefusal::code` |
| 4. Audit + alert path | Every rejection is durably audited through the `AuditSink` port into the P6.18 `FileAuditBuffer` (CRC-32 framing, disk-first appends, survives restart) — the drill's evidence-preservation step verifies the records ON DISK after the response. The prover emits `EDGE_PILOT_ATTACK_REJECTED` per rejected case (`PilotAttackCase::rejected_event_name`). The ALERT leg (audit event → operator alert → first-responder page) is the P9.25-verified alerting infrastructure — infrastructure-PENDING until P9.25 runs; the drill template carries that label per case. | `crates/edge-agent/src/ports.rs` :: `AuditSink`, `EdgeAuditEvent`; `crates/edge-agent/src/audit_buffer.rs` :: `FileAuditBuffer::record_event`; `crates/edge-agent/src/pilot_rejection_proof.rs` :: `PilotAttackCase::rejected_event_name`; P9.25 (NOT YET BUILT) |
| 5. Containment + notification | The site reversion to shadow/local control is an OPERATIONAL action under the P9.23 procedure (the mode is a deployment/per-site setting — no code path "reverts" itself; §21.4). The evidence that containment happened is the site's mode record plus the post-drill audit-chain anchor. The notification leg is the P9.23 procedure executed by people; the code contributes only the audit evidence the notification cites. | P9.23 (NOT YET WRITTEN — open dependency §1); `crates/edge-agent/src/audit_proof.rs` :: `compute_chain_anchor`, `verify_against_anchor` |

**Code-level rehearsal (pre-flight, not the drill):** the P9.31 suite
`crates/edge-agent/tests/pilot_rejection_proof_p9_31.rs` (9 tests, WRITTEN-
UNRUN) rehearses the MECHANICS of every case in-process — including the
fail-loud `AttackOutcome::ErroneouslyAccepted` recording and the
durably-audited-rejection join with the buffer. The drill pre-flight
REQUIRES that suite green against the pilot edge build first (§1). The
drill itself adds what no unit test can: real delivery, real people, real
clocks, real notification deadline.

## 4. Containment-evidence checklist (what a completed drill must file)

| # | Artifact | Expected content | From |
|---|---|---|---|
| E1 | Pre-drill audit-chain anchor | Head digest + sequence range + record count, retained platform-side | `audit_proof.rs` :: `compute_chain_anchor` |
| E2 | Pre-drill state snapshot | Sequence-checkpoint state, binding-journal count, buffer counters, site mode record | §2 step 1 |
| E3 | Inject record | Attack case code(s), delivery channel per case, injection instant(s) from the log | §2 step 2 |
| E4 | Detection record | First audit/log line (verbatim, with timestamp), detection time | §2 step 3 |
| E5 | Rejection evidence | The named rejection codes per case (expected: §3 leg 2), the `EDGE_*` audit lines, zero forwarder invocations | §3 legs 1–3 |
| E6 | Escalation + notification record | Who was called, when, notification instant vs P9.23 deadline (met/missed) | §2 step 4 |
| E7 | Containment record | Site ID reverted, to which mode, the mode-record reference, confirmation it is the INJECTED command's site | §2 step 5 |
| E8 | Post-drill evidence preservation | Post-drill chain anchor + verification against the drained records, preserved checkpoint/binding/telemetry artifacts, "nothing destroyed by the response" statement | §2 step 6 |
| E9 | After-action | Timings, P9.23 procedure corrections, disposition of any failure | §2 step 7 |
| E10 | Witness sign-off | The independent witness's identity/date and what they re-ran (human-gate field) | §5 |

## 5. The RUN record — EMPTY template (UNRUN; witness = human gate input)

All measured cells EMPTY. Nothing in this table may be filled except by the
human drill execution, from logs/audit timestamps (never from memory), with
the frozen versions recorded.

**Drill metadata (all EMPTY):** site ID / customer / operator present /
first responder / witness / frozen edge build + versions / P9.23 procedure
version used / P9.23 notification deadline for the class.

| Attack case | Injection used (live vs harness) | Detection time (measured) | Rejection code observed (expected: §3) | Zero adapter reach confirmed? | Alert path exercised? (P9.25) | Notification deadline met? | Containment: site reverted (ID + mode) | Evidence preserved before remediation? | Witness (human gate) |
|---|---|---|---|---|---|---|---|---|---|
| forged_signature | (EMPTY — UNRUN) | | | | | | | | |
| replayed_captured | (EMPTY — UNRUN) | | | | | | | | |
| expired_window | (EMPTY — UNRUN) | | | | | | | | |
| out_of_sequence | (EMPTY — UNRUN) | | | | | | | | |
| conflicting_idempotency_payload | (EMPTY — UNRUN) | | | | | | | | |

**Acceptance rule (recorded, from the ledger):** the drill closes P9.36
only when (a) the attempt was detected and contained by the P9.23 procedure
AS WRITTEN, (b) the affected site was reverted to shadow/local control and
the reversion is confirmed in the mode record, (c) the customer
notification met the recorded deadline, (d) evidence was preserved before
remediation, and (e) procedure corrections are applied to P9.23. Any
injected case NOT rejected with its expected code is a REAL finding —
fail-loud, fixed by its owning owner, and re-drilled; it is never
recorded as a pass, coerced, or explained away.

## 6. What a successful drill would and would not establish

**Would establish (once run):** for the five injected cases, at the named
pilot site, on the frozen versions: the containment chain worked END TO END
— the forged/replayed/expired/out-of-sequence/conflicting payload was
rejected by the cited code with the named code, the rejection was durably
audited and preserved, the P9.23 procedure was executable as written by
real people within its recorded timings, the customer was notified inside
the deadline, the site was reverted to the correct mode, and nothing was
destroyed by the response.

**Would NOT establish (§13.3 discipline — say it, don't imply it):**

- A five-case inject is a FINITE test set. Zero containment misses in the
  drill is a TEST OUTCOME, not a guarantee that no injection outside the
  `PilotAttackCase` vocabulary can pass or that the procedure contains
  every incident class.
- It does not certify the edge agent, the platform, the site, or any
  adapter (§17.5's own closing sentence; AGENTS.md §3 rule 2).
- It does not strengthen the underlying primitives: the `edge-sig:` scheme
  is still the recorded keyed-checksum (tamper-EVIDENCE against casual
  tampering, NOT cryptographic proof — the P6.15 honesty bound stands), the
  audit chain is FNV-1a-64 (P9.32's tamper-evidence bound), and the nonce
  derivation is test-grade (P9.33). The replacements remain the recorded
  deployment requirements; the drill exercises the mechanisms as they are.
- It does not prove the site-local protection is independent of FlexCore
  (invariant 10 — P9.20's drills, not this one).
- It does not establish bounded automation, field write authority beyond
  the §21.4 ladder, or any control-scope widening (ENTRY_NOTE boundary 3).
- One site, one drill: P9.06-style site-specificity applies — a drill at
  site A is not evidence for site B.

## 7. Scaffolding decision (recorded, not silent)

The ledger P9.36 definition dictates NO code: its deliverable is "Live
incident-drill record", and ENTRY_NOTE boundary 6 classifies it as
TEMPLATE + procedure. The inject vocabulary, the fail-loud acceptance
recording, and the code-level rehearsal suite already exist from P9.31
(`PilotAttackCase`, `PilotRejectionProver::run_attack`,
`tests/pilot_rejection_proof_p9_31.rs`). **No new edge-agent test file is
added this wave**: a second suite over the same five cases would duplicate
an UNRUN surface, and the drill's live legs (delivery, detection,
notification, containment) are human actions no unit test can express.
This record is the deliverable, per the ENTRY_NOTE discipline.

## 8. Reviewer-verification procedure (human)

1. Confirm this record's EMPTY state: no cell of §5 is filled, no timing is
   claimed, no witness is named — a filled cell here without a human-gate
   record elsewhere is a fabrication defect.
2. Trace every §3 citation (`grep -n "<symbol>" <file>`); every cited
   symbol must exist.
3. Confirm the §1 open dependencies are real: no P9.23 record exists in
   this directory; the P9.31 matrix is ticked but its tests remain UNRUN
   until the human build/test pass.
4. When the drill runs: fill §5 ONLY from logs/audit timestamps; file the
   E1–E10 artifacts of §4; apply the acceptance rule verbatim; the witness
   re-runs at least one case from this plan alone (the ledger's
   verification-method analogue: "inspect the drill record for timings and
   the customer notification; confirm the reversion in the site's mode
   record").
5. Confirm no safety/certification wording and no invented site, customer,
   date, or sign-off anywhere in this record.

## 9. Phase 7 follow-ups and open risks for P9.27

**Follow-ups carried (owner recorded; never closed by this record):**

1. The platform-side §14.3 idempotency-key registry — the drill's
   `conflicting_idempotency_payload` case exercises the EDGE-LOCAL stand-in
   (`FileCommandIdBinding`); the authoritative cross-edge control remains
   the Phase 7 contract gap (`31_edge_rejection_proof.md` §7.1). Owner:
   platform engineering.
2. Cryptographic signature replacement (the `edge-sig:` deployment
   requirement) — the drill's forged_signature case proves the CURRENT
   mechanism only. Owner: P9.09 (pilot key inventory) + platform
   engineering.
3. Sequence-density decision (head gaps admitted) — the drill's
   out_of_sequence case exercises the recorded monotonic-window design.
   Owner: Phase 7 contract owner (`31_edge_rejection_proof.md` §7.3).
4. Binding-journal rotation/compaction procedure — affects drill evidence
   retention windows. Owner: Phase 7 operations contract.

**Open risks recorded for P9.27:**

| # | Risk | Basis |
|---|---|---|
| R-P9.36-1 | P9.23 (procedure + tabletop) does not exist yet; the drill is UNSCHEDULABLE until it does — a schedule slip in P9.23 directly slips P9.36 and P9.28 | §1 |
| R-P9.36-2 | The alert-path leg depends on P9.25-verified alerting infrastructure (not yet built); a drill run before P9.25 would have an unverified alert leg — recorded as infrastructure-PENDING per case | §1, §3 leg 4, §5 |
| R-P9.36-3 | The drill proves containment of the FIVE recorded attack cases only; incidents outside the closed `PilotAttackCase` vocabulary are untested by construction | §6 |
| R-P9.36-4 | The forged/replayed defence rests on integrity-checksum-grade primitives until the recorded cryptographic replacements land; the drill would be re-run after any such replacement (a format-visible change) | §6, §9.2 |
| R-P9.36-5 | Site-specificity: a drill at one named site does not transfer to another; multi-site pilots need per-site runs (the ledger's "reverting the wrong site" failure mode lives here) | §2 step 5, §6 |
| R-P9.36-6 | The invariant-9 composition map-gap carried from Phase 7 (see `35_safety_invariant_map.md` §2) remains OPEN and is referenced here because the drill's rejection codes are produced by the same Phase 6 controls that map proves | §3, the P9.35 record |
