# P8.34 — Proposal validity windows and stale-proposal rejection

Date: 2026-11-14. Subtask: P8.34 (`ledger_phase_8.md` §P8.34). Owner
role: Research lead + Rust engineer. Report trace: §14.3
(valid-from/valid-until on every command); §17.5 invariant 5 (no stale
or expired command); §13.4. Inputs: P8.04 contracts
(`04_worker_contracts.md`); P8.05 harness
(`crates/flexcore-research/src/validation.rs` — whose header records
the P8.34 seam this subtask completes); the kernel checkpoint pattern
extended BY CITATION:
`crates/flexcore-kernel/src/revocation.rs` (`FileRevocationRegister` —
append-only framing, CRC-32, truncate-and-audit, durability before
state); the implementation
`crates/flexcore-research/src/proposal_windows.rs` and its integration
tests
`crates/flexcore-research/tests/proposal_windows_and_tenant_isolation.rs`.

## 1. Status of this record — WRITTEN-UNCOMPILED, nothing run

The implementation and its test suite exist and were written under the
loop bans: nothing installed, nothing built, no cargo command executed,
no test run. Every expected value in the tests is hand-computed from
the recorded rules (window boundaries, frame sizes, refusal names) and
stated in the test comments. NOTHING here is evidence that the crate
compiles or that any test passes; the human build/test gate (AGENTS.md
rule 7) owns that evidence. The ledger tick means *implemented,
awaiting human build/test*.

## 2. The recorded rule (stated plainly, including exactly-at-expiry)

Recorded verbatim in code as `proposal_windows::WINDOW_RULE` and pinned
by tests:

> a research proposal is current iff `valid_from_ms <= now <
> valid_until_ms` on the INJECTED clock. Exactly-at-open
> (`now == valid_from_ms`) is **ACCEPTED** (the window opens
> inclusive); exactly-at-expiry (`now == valid_until_ms`) is
> **REJECTED** (the window closes exclusive). The window is granted by
> the HOST from the injected clock — never read from the proposal's own
> clock — its duration is capped, and freshness is re-checked at the
> point of use, not only at proposal time.

This is ONE rule in three places: the kernel `PluginRequestEnvelope`
window semantics (§14.3 field 15 by analogy), the harness stage 4
(`validation.rs::check_window`), and the consumption-point re-check
(`proposal_windows::gate_at_time_of_use`) — the consistency is pinned
by the test `gate_at_time_of_use_matches_the_harness_stage_decision`
(hand-computed: at `now == 2_000` on window [1_000, 2_000), the
harness and the consumption check return the IDENTICAL refusal
`PROPOSAL_EXPIRED`).

## 3. What was implemented (ledger steps 1–2)

| Piece | Where | Ledger step | Notes |
|---|---|---|---|
| Host-side window grantor | `grant_validity_window(clock, duration)` | 1 | The clock is INJECTED (`validation::HarnessClock` — the crate's one clock vocabulary; `SystemTime::now()` appears nowhere); the duration is capped at `MAX_VALIDITY_WINDOW_MS` = 3_600_000 ms (one hour — RECORDED DEFAULT, RECOMMENDATION; re-litigable only through a new recorded decision). Non-positive → `PROPOSAL_WINDOW_DURATION_NON_POSITIVE`; over-cap → `PROPOSAL_WINDOW_EXCEEDS_CAP`. The ledger's recorded risk "a window set from the proposal's own clock with no bound" is closed structurally: there is no constructor that takes a window from a proposal |
| Wire-contract window extraction | `window_from_wire_parts(Option<i64>, Option<i64>)` | 1 | Either field absent → `PROPOSAL_WINDOW_MISSING` (the absent field named) — a missing window is refused, NEVER defaulted; inverted → `PROPOSAL_WINDOW_INVERTED`. The proposal type's `validity` field is non-optional, so a missing window can only arrive at the wire — this is the extraction point that refuses it |
| Snapshot-freshness SOURCE | `CheckpointedSnapshotSource` implementing `validation::SnapshotFreshnessSource` | 2 | Superseded (replacing id travels) / Current (known, not superseded) / Unknown (never fresh — the P4.11 rule). Fail-closed: a source that cannot read its register never reports Current |
| Restart-surviving supersession checkpoint | `MemorySnapshotSupersessionRegister` + `FileSnapshotSupersessionRegister` (append-only `SnapshotSupersessionStore` port) | 2 | The kernel revocation-checkpoint pattern mirrored by citation (no flexcore-kernel dependency): magic `FLEXCORE-RESEARCH-SNAP-REG` + version 0x01, frames `[u32 LE len][u32 LE crc32][payload]`, truncate-and-audit recovery with a COUNTED loss counter, durability before state. A snapshot superseded before a restart stays superseded after it |
| Replay/supersede handling | `supersede(old, new, actor, clock)`; chains A→B→C | 2 | Append-only: one record per superseded id (duplicate → `SNAPSHOT_ALREADY_SUPERSEDED`); self-supersession and empty ids → `SNAPSHOT_SUPERSESSION_INVALID`; empty actor → `SNAPSHOT_SUPERSESSION_ACTOR_REQUIRED`; timestamps injected only. Freshness reads the DIRECT record, so a chained stale proposal is refused with `by = B` and the consumer sees the chain through repeated checks |
| Gate-at-time-of-use | `gate_at_time_of_use(proposal, clock, snapshots)` | 2 | The consumption-point re-check: returns the harness's OWN refusal vocabulary (`PROPOSAL_NOT_YET_VALID` / `PROPOSAL_EXPIRED` / `SNAPSHOT_SUPERSEDED` / `SNAPSHOT_UNKNOWN`), so a stale proposal is rejected by the same gate class as an infeasible one, BEFORE any downstream consumption. Pure and side-effect-free; never calls the physical gate (pinned by `the_consumption_check_never_consumes_a_gate_attempt`) |

## 4. Harness integration — ADDITIVE; the stage list is UNCHANGED (the versioned-change determination, recorded)

The P8.05 harness's fixed stage order (`STAGE_ORDER`) and version
(`HARNESS_VERSION = "flexcore-research/validation.v1"`) are **NOT
changed** by this subtask. The determination, recorded explicitly so it
is never silent: validation.rs stages 4–5 ALREADY enforce the window
and snapshot freshness at validation time (its header reserved exactly
this seam); what lands in P8.34 is (a) the host-side grantor, (b) the
freshness SOURCE the composition root injects into the harness, and (c)
the consumption-point re-check — all OUTSIDE the stage list. The test
`the_stage_list_is_unchanged_by_this_module` pins both constants.

Had the stage list needed to change, that would have been a NEW
`HARNESS_VERSION` (the P8.06 frozen-version discipline applied to the
harness itself — never a silent edit of what "validated" meant). It did
not. No `validation.rs` byte is touched by this subtask (file ownership
kept additive), and the stale-proposal rejections of §5 flow through
the EXISTING stage order.

## 5. The hostile-proposal test record (ledger step 3) and the contract gap

The ledger's three hostile proposals, implemented in
`crates/flexcore-research/tests/proposal_windows_and_tenant_isolation.rs`
(WRITTEN-UNRUN; the ledger's verification method — "submit the three
hostile proposals and record the rejections" — is the human build/test
gate plus a reviewer re-run):

| Hostile proposal | Test | Expected rejection (hand-computed) | Gate consumed? |
|---|---|---|---|
| Expired (submitted at `now == 2_000`, window [1_000, 2_000)) | `hostile_proposal_1_expired_is_rejected_and_never_reaches_the_gate` | `PROPOSAL_EXPIRED` from the harness AND from the consumption check | NO — 0 gate calls |
| Against a superseded snapshot (a→b recorded, proposal computed against a) | `hostile_proposal_2_superseded_snapshot_is_rejected_and_never_reaches_the_gate` | `SNAPSHOT_SUPERSEDED { by_snapshot_id: "snap-2026-11-14-b" }` | NO — 0 gate calls |
| Missing window (wire fields absent) | `hostile_proposal_3_missing_window_is_refused_never_defaulted` | `PROPOSAL_WINDOW_MISSING` at the extraction point — no defaulted window can be constructed | n/a (pre-construction) |

Plus the happy path (`a_fresh_window_from_the_grantor_flows_through_
the_full_harness`: granted window [1_000, 1_500), full harness pass,
gate consumed exactly once, consumption check agrees) and the
in-module `#[cfg(test)]` suite in `proposal_windows.rs` (boundary
known-answers at 999/1_000/1_999/2_000; grantor cap edges; missing/
inverted wire windows; freshness classification; supersession
validation; file-register round-trip, foreign-file refusal, and
corrupt-tail counting).

**The contract gap (recorded, never invented-fixed — ENTRY_NOTE
boundary 3):** `python/workers/contracts/worker_api.proto` does NOT
carry `valid_from_ms` / `valid_until_ms` on the result envelope —
`ResultProvenance` carries `input_snapshot_id` already (so the
snapshot half of the contract exists), but the window half travels
only in the Rust proposal type and the host-side grantor. Recorded as
a Phase 7 follow-up:

| # | Phase 7 follow-up | Owner / activation point |
|---|---|---|
| WF-1 | Add the validity-window fields to the worker result envelope (or a recorded decision NOT to carry them on the wire, with the host-side grantor as the standing mechanism) — a Phase 7 contract extension with buf governance (§10.5), never a parallel contract | Phase 7 contract owners; at the next recorded contract pass (P8.28 CI gates or the P7.27 activation) |

## 6. Reviewer verification procedure

1. Re-run the hostile-proposal suite (the ledger's verification
   method): `cargo test -p flexcore-research` after the human build
   gate; the three test names in §5 must appear and pass.
2. Confirm the exactly-at-expiry rule from the test itself: the
   boundary table asserts `now == 2_000` → `PROPOSAL_EXPIRED` and
   `now == 1_000` → accepted, on window [1_000, 2_000).
3. Confirm `SystemTime` appears nowhere in `proposal_windows.rs` or
   its tests (grep) — the window is computed only from the injected
   `HarnessClock`.
4. Confirm `validation.rs` is untouched (git diff) and that
   `STAGE_ORDER`/`HARNESS_VERSION` are pinned unchanged by
   `the_stage_list_is_unchanged_by_this_module`.
5. Confirm the file register's restart property from the test
   `file_register_round_trips_and_survives_a_reopen` (supersession
   survives a reopen; `corrupt_tail_truncated()` counts, never silent)
   and that a foreign-magic file is a hard open error.
6. Confirm `gate_at_time_of_use` is pure (no audit port, no gate
   reference) and returns `validation::Refusal` values — the same gate
   vocabulary, per §3.

## 7. Honest status

**WRITTEN-UNCOMPILED.** No cargo command was executed in this loop.
The hostile-proposal record above is the WRITTEN suite with
hand-computed expectations, not a run record — no run output cell is
filled anywhere in this file. flexcore-kernel is untouched; the
checkpoint pattern is mirrored by citation with the alignment seam
recorded in code (`KERNEL_CHECKPOINT_ALIGNMENT_SEAM`).
