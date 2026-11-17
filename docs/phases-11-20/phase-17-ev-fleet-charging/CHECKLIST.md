# Phase 17 — EV Fleet and Charging Integration: Exit-Gate Checklist

**Exit gate (plan §6, the phase's own acceptance criteria): every subtask
has a decision record; the deliverables are adapter design + message
mappings + composer specification + written-unrun test files + simulator
profile specifications + assessment/decision records — no charge point is
ever connected, no OCPP message is ever exchanged with a real device, no
certification is claimed; charging profiles enter the lifecycle at PROPOSED
and pass all three gates, the never-granted permission set stays
byte-identical; the dependency checker stays green, `verify_docs.py` and
`validate_agent.py` report no failures; P17-E-1, P17-E-2, and P17-E-3 remain
OPEN with DEFAULTs recorded; D5 remains OPEN and human-closed; no
conformance, certification, metrology, or safety claim appears anywhere.**

Each item states the evidence and its status. A checked item without
evidence is not checked. **Nothing in this phase has been built, run,
connected, issued, or certified** — the first human build is the program
gate; every status below is a written-artifact status, never a runtime
claim.

## Plan §6 acceptance criteria (one-for-one)

- [x] **Criterion 1 — every subtask has a decision record under `records/`
  citing files touched and [S-n] sources.** Evidence: 18/18 coverage table
  below; every record carries a Sources field and a Files-touched field;
  every external claim carries [S-n] or an UNVERIFIED/ASSUMPTION label
  (the close record §3.3 audited all 22 distinct S-ids used — all resolve
  in `00_GAP_ANALYSIS.md` §11). Status: **satisfied by the written
  records and the audit**.
- [x] **Criterion 2 — the deliverables are adapter design + message
  mappings + composer specification + test files written-unrun + simulator
  profile specifications + assessment/decision records; no charge point is
  ever connected, no OCPP message is ever exchanged with a real device, no
  certification is claimed.** Evidence: the designed UNRUN
  `CsmsTransport` seam (record 01 §7; `UnwiredCsmsTransport` refuses every
  operation); the only OCPP traffic in the program is the in-process
  simulated loopback (record 44 §2); the honesty sweep (close record §3.4)
  found the no-device/no-certification boundary stated in every record and
  zero conformance/certification claims. Status: **satisfied by the audit**
  as of the close record's date (an inspection, not a tool run).
- [x] **Criterion 3 — charging-profile payloads exist only downstream of
  the 14-state lifecycle's gates; the never-granted set is byte-identical
  before and after; adapters get dispatch reach only through the command
  lifecycle.** Evidence: `into_relay_request` builds the wire payload ONLY
  from `CommandState::Issued` (record 05 §4, pinned by the conformance
  suite); proposals and readiness records are structurally incapable of
  carrying a lifecycle state (records 21 §1, 42 §2); the never-granted set
  quoted verbatim and unchanged (close record §3.5; `manifest.rs`
  byte-identical, git diff empty); `command_state.rs` byte-identical.
  Status: **satisfied by the pinned tests and the audit**.
- [x] **Criterion 4 — `check_dependency_rule.py` stays green after every
  batch; `verify_docs.py` and `validate_agent.py` report no failures; every
  new crate is an additive workspace member.** Evidence: the verbatim
  outputs in close-out records 06, 27, 59, and 65, and the final close
  run (close record §6) — all green (EXIT=0 ×3); the one new crate
  (`crates/adapters/protocol-ocpp`) is an additive member with the
  additive checker entry. Status: **satisfied, quoted**.
- [x] **Criterion 5 — P17-E-1, P17-E-2, and P17-E-3 remain OPEN with
  DEFAULTs recorded; D5 remains OPEN and human-closed; no conformance,
  certification, metrology, or safety claim appears anywhere.** Evidence:
  record 63 §1 (both register locations verified, none edited, none
  closed); record 63 §3 (the D5 interplay note — the Edition-2 conflict
  cited, never resolved); the honesty sweep (close record §3.4). Status:
  **satisfied by the register compilation and the audit**.

## Subtask coverage (18/18 with decision records)

| Subtask | Record | Status |
|---|---|---|
| P17.01 | 01 | written-unrun |
| P17.02 | 02 (+ P17-E-2 registered OPEN) | written-unrun |
| P17.03 | 03 | written-unrun |
| P17.04 | 04 | written-unrun |
| P17.05 | 05 (+ `tests/ev_charging_composer.rs`) | written-unrun |
| P17.06 | 21 | written-unrun |
| P17.07 | 22 | written-unrun |
| P17.08 | 23 (+ P17-E-3 registered OPEN) | written-unrun |
| P17.09 | 24 | written-unrun |
| P17.10 | 25 (+ the structural negative tests) | written-unrun |
| P17.11 | 26 | written-unrun |
| P17.12 | 41 | record-only / phase-gated |
| P17.13 | 42 (+ `tests/ev_v2g_readiness.rs`) | written-unrun |
| P17.14 | 43 (+ `tests/conformance_suite.rs`) | written-unrun |
| P17.15 | 44 (+ `tests/p17_15_simulated_ocpp_charge_points.rs`) | written-unrun |
| P17.16 | 61 | assessment record (record-only depth) |
| P17.17 | 62 | decision record (record-only depth) |
| P17.18 | 63 (decision register) + 64 (close + audit) + 65 (batch close-out) | register compiled; E-decisions OPEN; close + consistency audit |

## Human-gate queue (NOT-STARTED / EMPTY / UNSIGNED)

1. First workspace human build + clippy + `cargo test --workspace` (covers
   every written-unrun suite, including the conformance suite and the
   simulator loopback suite).
2. The OCA-text verification pass over every UNVERIFIED register (records
   01/03/04/05/21/22/23/26/41/42/43) — a mismatch is a recorded edit,
   never a silent reinterpretation.
3. P17-E-1/E-2/E-3 review and closure by the human owner (record 63 §2
   names what closes each; every closer input is EMPTY / NOT-STARTED).
4. D5 closure by the human owner — AFTER the Edition-2 publication-date
   verification resolves (`ledger/phase-06-edge-protocol/RESEARCH.md` §3
   open verification task; cited in record 63 §3, unresolved).
5. The PnC ungate checklist (record 41 §3: named PKI operator, anchor
   distribution, OCSP service, OCA-text pass, D5) — all NOT-STARTED; no CA
   is stood up anywhere in this program.
6. The real WebSocket/TLS transport wiring decision (record 01 §7) — the
   simulated loopback (record 44) does not substitute for it.
7. X-8 pre-registration: any future conformance/lab run registers its
   thresholds BEFORE the run; no lab result, fee, or sign-off exists.
