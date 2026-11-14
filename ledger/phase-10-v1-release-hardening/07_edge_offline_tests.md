<!-- P10.07 — Edge offline/degraded-mode tests (decision record). REAL test files WRITTEN-UNRUN over the actual crate seams (ENTRY_NOTE boundary 3): zero observed failures are NOT claimed from an unrun suite; the run is the human build/test gate. Release-candidate identifier PENDING P10.18. Timings and versions for the §22.3 per-test table are human-gate cells (EMPTY). -->
# P10.07 — Edge Offline/Degraded-Mode Tests

Date: 2026-11-14. Subtask: P10.07 (`ledger_phase_10.md` §P10.07). Owner role:
Platform engineering (with integration engineering). Report trace: §22.3
release gate 6; §18.1–§18.3; §21.2; §17.4; §14.4. Inputs: P10.01 scope
freeze; the edge crate seams (`runner.rs` P6.05, `buffer_file.rs` P6.09,
`audit_buffer.rs` P6.18, `command_path.rs` P6.14, `command_verify.rs` P6.15,
`command_forward.rs` P6.16, `policy.rs` P6.17, `health.rs` P6.17,
`clock_quality.rs` P6.31, `pilot_rejection_proof.rs` P9.31); P9.20 fallback
drills and P9.24 backup rehearsal — CROSS-REFERENCED, not redone. Deliverable:
two additive test files in `crates/edge-agent/tests/`; no `src` logic touched;
no other crate touched.

## 1. Honest status — the plain statement

**The suite is WRITTEN, never executed.** The loop bans (no builds, no test
runs, no installs) were held; per ENTRY_NOTE boundary 3 this is
"implemented, awaiting human build/test". **Zero observed failures are NOT
claimed** — an unrun suite has no observed failures and no observed passes.
Every expected outcome is hand-computed from the named module's recorded
policy (capacity boundaries, age-retention comparisons, expiry windows,
tolerance inclusivity), so the run, when it happens, compares against an
independent statement of the code's own documented behaviour — not a
self-check. **No zero-loss or zero-duplication claim appears anywhere** in
the suite or this record: the tests assert the DOCUMENTED BOUNDED behaviours
(audited drop-oldest, at-least-once redelivery with possible downstream
duplicates, counted corruption truncation).

The suite also runs against a release candidate that **does not exist as a
built artifact yet** (ENTRY_NOTE boundary 1): the candidate identifier is
`<PENDING P10.18>`; any candidate change re-opens this subtask.

| Field | Value |
|---|---|
| Release-candidate identifier | `<PENDING P10.18 — no candidate build exists>` |
| Suite result | `<EMPTY — WRITTEN-UNRUN; the human run fills this>` |
| Per-test time-to-fallback / time-to-recovery | `<EMPTY — measured on the candidate; human-gate cells>` |
| Edge build version tested | `<EMPTY — must match the candidate identifier>` |
| P9.20 / P9.24 cross-reference | fallback drills / backup rehearsal TEMPLATE records — cited, not redone |

## 2. What was produced (files)

| File | Content |
|---|---|
| `crates/edge-agent/tests/p10_07_edge_offline_degraded_modes.rs` | The offline/degraded sweep over the REAL seams: (1) telemetry buffer offline (`FileLocalBuffer`, P6.09) — bounded capacity with the audited drop-of-OLDEST (hand-computed: capacity 2 → the 3rd append drops exactly one; counter + latched health flag), hand-computed max-age retention boundary (age == limit KEPT, one millis older DROPPED; the predicate `ingest < now − max_age`), store-and-forward across restart, the crash-in-drain-window redelivery (at-least-once — duplicates possible, consumed downstream; zero-duplication never claimed), failed-forward prepend-back in order, foreign journal refused not guessed; (2) audit buffer overflow (`FileAuditBuffer`, P6.18) — audited drop-oldest with the `appended == pending + dropped` reconciliation, persisted sequences, in-order upload with the fail-closed `OutOfOrderCommit` refusal (nothing removed), actor+object mandatory (the P6.21 shape), NO age-based audit drop (retention is a deployment RetentionPolicy decision, P0.16 §4); (3) uplink loss (`EdgeRunner`, P6.05) — degraded entry/exit audited exactly once each in order, a long outage is bounded by backpressure (full buffer PAUSES polling, audited ONCE per episode, nothing silently dropped), a source poll failure audited with no fabricated telemetry; (4) command path disabled (`CommandPathRuntime`/`CommandReceiver`, P6.14) — the default build refuses with `command_path_disabled`/`config_flag_off`, the refusal audited (`EDGE_COMMAND_REJECTED`) and nothing retained, the nine `GateRefusal` codes pairwise distinct, the fixed evaluation order pinned, the read-time expiry boundary hand-computed (`valid_until` instant already disabled, `−1` enabled); (5) central outage — the site-local policy engine (hand-computed rate-limit boundary: the 3rd command in the window refused, a refusal consumes no budget, budget renews exactly at the window age) and the health monitor (`FAILING_*` blocks command targeting locally; `DEGRADED_STALE_DATA` does not block) take NO uplink — §17.4/§21.2 "no dependence on cloud for protection", FlexCore supervisory only; (6) no stale-buffered command is issued — a full degraded buffer composes a receiver that still refuses everything (P9.24 cross-reference, acceptance c). |
| `crates/edge-agent/tests/p10_07_edge_command_verification_and_restart.rs` | The command-verification slice: (1) the P6.15 four-check verifier — forged (tampered/unsigned/wrong-signer/payload-binding-mismatch → `signature`), replayed (`duplicate`), expired (hand-computed window boundaries: `valid_until − 1` valid, `valid_until` expired, `valid_from − 1` not yet valid; expired authorization reference → `expiry`), out-of-sequence/unsequenced (`sequence`), value/asset/quantity outside the approved scope (`scope`), non-allowlisted adapter (`allowlist`) — each with its DISTINCT named code; a refused command never advances the sequence window (the P4.28 SequenceGuard rule through the recorded fixed order); (2) the clock-drift hold (P6.31 + the verifier seam) — a Degraded/Unknown/Unsynchronized clock fails the expiry check CLOSED for otherwise-valid signed commands (the §17.4 "expired command accepted because the edge clock drifted" risk closes in the safe direction), the hold audited (`EDGE_CLOCK_HOLD_ON_DRIFT`), tolerance inclusive at exactly the pilot DEFAULT 100 ms, Degraded at 101 ms, sync-timeout boundary 30 000/30 001 ms, a corrupt pair (offset ≥ one day) refused never estimated; (3) restart with checkpoint survival — the `FileSequenceCheckpoint` journal survives restart and a POST-RESTART replay of an accepted command is refused (`duplicate`; the ledger's restart-replay risk closed), a corrupt checkpoint tail truncated and COUNTED, the `FileCommandIdBinding` idempotency journal survives restart and refuses a conflicting payload under a used id (`ConflictingIdempotencyPayload`); (4) the P9.31 pilot rejection prover re-exercised — all five attack cases rejected with their expected codes, zero acceptances (a finite test set, not a guarantee); (5) the §14.4 fallback discipline — the closed fallback option set pinned VERBATIM (exactly `HOLD_LAST_ACCEPTED_SETPOINT` \| `RETURN_TO_LOCAL_EMS_AUTONOMOUS` \| `PRESERVE_RESERVE_SOC_CONSTRAINTS_AND_STOP_ACCEPTING_NEW_COMMANDS` \| `REQUIRE_OPERATOR_INTERVENTION`), the no-universal-"set power to zero"-fallback clause asserted verbatim in the unknown-variant refusal, HOLD requires a positive BOUNDED interval (unbounded hold forbidden), a policy without its operator review cannot load, and no gate input combination defaults to enabled. |

## 3. Per-test record table (§22.3 step 5: trigger → observed behaviour → timing → versions)

The trigger and the expected (hand-computed) behaviour are IN the suite; the
measured columns are human-gate cells on the candidate build:

| # | Degraded mode (§18.1/§21.2) | Trigger (in the suite) | Observed behaviour (asserted) | Time to fallback/recovery | Versions |
|---|---|---|---|---|---|
| 1 | Telemetry buffer offline | uplink down; samples accumulate | bounded; audited drop-oldest at the bound; in-order store-and-forward | `<EMPTY>` | `<EMPTY>` |
| 2 | Audit buffer behaviour offline | audit events exceed the journal bound | audited drop-oldest; counter + latched flag + reconciliation; in-order upload | `<EMPTY>` | `<EMPTY>` |
| 3 | Link loss / recovery | forward failures, then success | degraded entered/exited, audited once each; drain in order | `<EMPTY>` | `<EMPTY>` |
| 4 | Long outage (bounded buffering) | channel never recovers | backpressure pauses polling at the bound; audited once; no silent drop | `<EMPTY>` | `<EMPTY>` |
| 5 | Adapter failure | source poll fails | audited; nothing fabricated; health FAILING blocks command targeting locally | `<EMPTY>` | `<EMPTY>` |
| 6 | Certificate expiry | credential at/past its expiry input | FAILING_CREDENTIAL_EXPIRY blocks dependent actions (health seam) | `<EMPTY>` | `<EMPTY>` |
| 7 | Command path disabled | default gate inputs; expiry | every command refused + audited; nothing retained; read-time expiry | `<EMPTY>` | `<EMPTY>` |
| 8 | Clock drift | offset beyond tolerance / source quiet | expiry checks fail CLOSED (hold-on-drift, audited) | `<EMPTY>` | `<EMPTY>` |
| 9 | Central outage | uplink absent (structural) | site policy + health still enforced locally; site EMS protective behaviour local | `<EMPTY>` | `<EMPTY>` |
| 10 | Forged/replayed/expired command | P6.15 verifier injections | distinct named rejections before any forwarding; P9.31 prover zero acceptances | `<EMPTY>` | `<EMPTY>` |
| 11 | Restart with checkpoint | process restart, journals replayed | replay-after-restart refused; corruption truncated + counted; idempotency bindings intact | `<EMPTY>` | `<EMPTY>` |

Note on §18.1's "remote upgrade with staged/rollback capability": the edge
UPDATE mechanism (`update_verification.rs`, P9.34) is a separate seam; the
degraded-mode suite covers the offline/buffer/command/health surfaces the
ledger names and does not duplicate the P9.34 update-verification suite —
cross-referenced, not redone.

## 4. Reviewer-verification procedure (the human gate)

```sh
cargo test -p flexcore-edge-agent --test p10_07_edge_offline_degraded_modes
cargo test -p flexcore-edge-agent --test p10_07_edge_command_verification_and_restart
# Independent re-run (P10.07 verification method): re-run the link-loss test
# against the release candidate once P10.18 records the identifier, and
# inspect the buffer/forward journal + audit trail for the audited episode
# (one EDGE_DEGRADED_BUFFERING_ENTERED, one EXITED, the drop counters, the
# rejection records). Confirm the edge build version matches the candidate.
```

## 5. Acceptance-criteria coverage and honest gaps

| P10.07 criterion | Covered here | NOT executable in this loop |
|---|---|---|
| (a) every listed degraded mode has a passing test on the candidate edge build | All modes have WRITTEN tests citing the real code path (§3 rows 1–11) | The PASS results need the built candidate; the per-test TIMINGS and versions are `<EMPTY>` human-gate cells |
| (b) offline buffering is bounded and store-and-forward is verified | Proven structurally (the bound is the configuration capacity; overflow is audited drop-oldest; the journal survives restart; commit compacts) | A LONG-duration soak (hours of outage) on real hardware is a deployment-run observation (P9.20's drills) — not invented here |
| (c) no stale buffered command is issued | The buffering/command-path independence test (§2 file 1, part 6) + the P6.14 disabled-by-default structural proof (no queue field, no setter) | — |
| (d) a forged/replayed/expired command is rejected at the edge | The four-check verifier slice + the P9.31 prover re-exercise, with distinct named codes and audit records | Zero observed misses is a TEST outcome of an unrun suite — claimed only after the human run |
| Risk: "an expired command accepted because the edge clock drifted (§17.4)" | The hold-on-drift tests: ANY non-synchronized clock fails expiry CLOSED — the risk closes in the safe direction | — |
| Risk: "an unbounded buffer that exhausts local storage on a long outage" | The backpressure + bounded-capacity tests; the drop-oldest counter is the audited loss report | The storage ceiling in bytes on the target gateway is a deployment-profile cell |

## 6. Findings (recorded, not fixed — src logic is outside this subtask's ownership)

- **F-P10.07-1 (observation, no action taken):** the at-least-once window
  between `drain` and `commit_forwarded` means a crash mid-forward
  RE-DELIVERS a batch after restart — downstream duplicates are possible by
  design and are consumed by the platform's idempotent telemetry ingest
  (the recorded P6.09 semantics). The suite asserts THIS (redelivery with
  order preserved), never zero-duplication.
- **F-P10.07-2 (observation, no action taken):** the audited drop-oldest
  overflow policy makes buffer capacity the knob that trades bounded loss
  against local storage. The telemetry buffer supports age retention; the
  AUDIT buffer deliberately does NOT (audit evidence retention is a
  deployment RetentionPolicy decision, P0.16 §4) — so a very long outage
  with a small audit-journal bound will drop OLDEST audit records with only
  the counter/flag as the loss report. Sizing the audit journal for the
  pilot's outage budget is a deployment-profile obligation (recorded, not
  decided here).
- **F-P10.07-3 (recorded constraint):** encryption at rest for the journals
  (§18.3) remains the recorded open item (P6.09/P6.18 decision docs) — the
  journals hold records in the clear on the edge host until closed with the
  P6.02-audited crypto dependency; the deployment profile must treat the
  journal paths as in-scope for OS-level site protection.
- **F-P10.07-4 (recorded constraint):** the edge command signature is the
  recorded P6.15 keyed-checksum scheme (tamper-EVIDENCE against casual
  tampering, NOT a cryptographic guarantee); replacing it under reviewed key
  management is the recorded deployment requirement. The verification tests
  prove the REJECTION mechanics, not cryptographic strength.
- **F-P10.07-5 (defect in an EXISTING test, recorded — src/test logic
  outside this subtask's ownership, not fixed):** the P6.31 unit test
  `the_tolerance_boundary_is_respected_inclusively` in
  `crates/edge-agent/src/clock_quality.rs` (lines ~703–712) observes offsets
  100 and 101 in sequence and asserts the second observation yields
  `Degraded` — but the monitor's even-count MEDIAN arithmetic
  (`median_offset`: `lower + (upper − lower) / 2` = 100 + (101 − 100)/2 =
  100 + 0 = 100, integer division) still estimates 100 ms, which is
  inclusive at the 100 ms tolerance, so the derived state after the two
  observations is `Synchronized` and the assertion would FAIL on the first
  human run. The boundary itself (inclusive at 100, Degraded at 101 for a
  single-pair estimate) is what the implementation computes; the test's
  second probe is constructed against a single-sample mental model. The
  P10.07 suite probes the boundary with single-pair monitors
  (`a_degraded_clock_holds_every_command_fail_closed_even_when_fully_valid`)
  and documents the median behaviour in its comment. Fix options (for the
  owner of that file): probe 101 on a fresh monitor, or use a
  three-sample window (100, 101, 101 → median 101 → Degraded).

## 7. Ledger tick

`P10.07` in `ledger_phase_10.md` was ticked in the same wave as this record,
with the ENTRY_NOTE boundary 3 meaning: **implemented, awaiting human
build/test**. The §3 timing/version cells and the P9.20/P9.24 cross-referenced
deployment observations remain open human-gate steps, not done.
