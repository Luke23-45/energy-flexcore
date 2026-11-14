# P9.32 — Audit integrity and edge audit-buffer proof: PROOF SURFACE IMPLEMENTED (awaiting human build/test)

Date: 2026-11-14. Subtask: P9.32 (`ledger_phase_9.md` §P9.32). Owner role:
Security / platform engineering. Report trace: §23 Stage 9; §17.3 (audit
logging); §18.1 (local audit buffer); §16.3 (retention); §22.3. Inputs:
the Phase 6 audit controls (`audit_buffer.rs`, P6.18, and the
`18_21_audit_offline_isolation.md` discipline); `ENTRY_NOTE.md` boundary 5
(REAL code + tests in `crates/edge-agent`, WRITTEN-UNRUN).

**Run-status honesty (AGENTS.md rule 7):** the code and tests in this wave
are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass
executes it. Nothing here claims a tamper test ran, an outage test ran, or
that a retention job was verified — those executions are the human gate
(also feeding P9.24 backup/restore rehearsal and P9.29's authority pack).

Files (complete list; nothing outside it was touched):

| File | Status | Content |
|---|---|---|
| `crates/edge-agent/src/audit_proof.rs` | NEW | The FNV-1a-64 hash chain over the drained `BufferedAuditRecord`s; the platform-retained `ChainAnchor` + named per-field mismatch verification; the authority-review `AuditExportBundle` + `VerifiedAuditExport` evidence record; the honest tamper-EVIDENCE bound |
| `crates/edge-agent/tests/audit_integrity_proof_p9_32.rs` | NEW, UNRUN | The 9-test tamper + outage matrix |
| `crates/edge-agent/src/lib.rs` | ADDITIVE only | Two `pub mod` lines + comments (shared with the P9.31 wave); no existing line changed |
| `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | P9.32 checkbox tick only | Exact-match replacement, asserted count 1 before and after |

## 1. The integrity mechanism (ledger step 1) — chain + platform-retained anchor

**Proven by exercise, not re-implemented:** the durable audit buffer under
test is the EXISTING P6.18 `FileAuditBuffer`
(`crates/edge-agent/src/audit_buffer.rs`) — CRC-32 length-prefixed framing,
disk-first `sync_data` appends, truncate-and-audit recovery with a counted
loss, audited drop-of-oldest overflow (hard counter + latched
`has_overflowed` flag), persisted monotonic sequences, in-order at-least-once
upload, and the fail-closed out-of-order commit refusal
(`AuditUploadError::OutOfOrderCommit` removes nothing). The record shape is
the EXISTING `BufferedAuditRecord` (sequence, `recorded_at`, kind, actor,
object, detail — the P6.21 actor+object+timestamp audit requirement).

**Added (ADDITIVE, new module `audit_proof.rs`):** the hash chain and the
anchor.

- `recompute_chain_head` folds each record — previous digest FIRST, then
  sequence, `recorded_at` millis, and length-prefixed actor/object/detail —
  into an FNV-1a-64 running digest. Because the previous digest is an
  input, the head commits to the whole ORDERED history: any edit, deletion,
  reorder, or insertion changes the head. The chain genesis is the fixed
  constant 0 (domain-separated by construction of the fold).
- `compute_chain_anchor` produces the `ChainAnchor` (version, algorithm,
  site, head digest, first/last persisted sequence, record count, computed
  instant from the INJECTED clock — never wall time).
- **The anchor is retained by the CENTRAL PLATFORM at export time** (the
  Phase 6 offline-isolation discipline: the platform keeps what the edge
  host cannot be trusted to keep). Later verification —
  `verify_against_anchor` — recomputes the chain over any claimed copy and
  compares EVERY anchor field; every mismatch is a NAMED error:

| Tamper class | Named detection |
|---|---|
| Content edit of any record | `AuditChainError::HeadMismatch` |
| Delete a middle or tail record | `AuditChainError::CountMismatch` (range check as second net) |
| Reorder records | `AuditChainError::NonMonotonicSequence` (the disorder itself is the finding) |
| Duplicate a record | `AuditChainError::CountMismatch` |
| Pad/prepend a different sequence range | `AuditChainError::SequenceRangeMismatch` |
| Empty claimed copy vs a retained anchor | `AuditChainError::CountMismatch` (never a vacuous pass) |
| Wrong chain version / algorithm / site binding | `AnchorVersionMismatch` / `AnchorAlgorithmMismatch` / `AnchorSiteMismatch` (refused, never guessed) |
| Empty record set offered for anchoring | `AuditChainError::EmptyRecords` (an empty export is not evidence) |

## 2. THE HONESTY BOUND — tamper-EVIDENCE, not tamper-PROOF

This is stated in the module header, in the anchor's own doc comment, in
the `audit_text()` strings an authority reads, and here — because a
security record that implies more than it proves is worse than useless:

- The chain digest is **FNV-1a 64** — the repo's existing NON-cryptographic
  hash family (the P4.28 `sim-sig:`, P6.14 enablement, P6.15 `edge-sig:`
  patterns). It is an integrity checksum: it reliably detects accidental
  corruption and any tampering that does not include a full chain
  recomputation.
- **There is NO SECRET in the chain.** A determined adversary WITH
  SUSTAINED WRITE ACCESS to the edge host can recompute a self-consistent
  chain over altered records and re-export a copy that passes a naive
  head-check. Detection then rests entirely on the anchor having been
  retained at the PLATFORM before the host was compromised, and on the
  upload channel delivering it. That is tamper-EVIDENCE with an explicit
  residual risk, NOT tamper-PROOF.
- Replacing the digest with HMAC-SHA-256 (or better) under reviewed key
  management is a recorded DEPLOYMENT REQUIREMENT, exactly parallel to the
  P6.15 command-signature honesty note. The versioned `algorithm` field
  makes that replacement a detectable format change, never a silent one.
- A test suite is a FINITE test set: zero observed tampering-missed in the
  suite below is a TEST OUTCOME, not a guarantee about tamperings outside
  the suite.

## 3. The outage proof (ledger step 3) — what the buffer already guarantees

The WRITTEN-UNRUN tests drive the EXISTING `FileAuditBuffer` through the
ledger's outage scenario; the guarantees are its recorded P6.18 behaviour,
proven by exercise:

| Ledger requirement | Buffer mechanism exercised | Test |
|---|---|---|
| No loss during a central outage | disk-first `sync_data` appends; truncate-and-audit recovery | `buffered_audit_events_survive_an_outage_and_upload_exactly_once` (7 events, crash mid-outage, restart) |
| Upload exactly once (no duplication) | records persist until `commit_uploaded`; the commit compacts the journal to the pending set | same test, phase 4 reopens and asserts zero pending |
| In order | `drain` always takes from the FRONT; persisted monotonic sequences continue across restarts | same test (sequences 1..=7) and `a_crash_in_the_upload_window_redelivers_at_least_once_in_order` |
| Crash in the drain→upload→commit window | the window is at-least-once (§15.4); duplicates consumed platform-side (P4.24 discipline, cited) | the crash-window test (drain without commit, restart, all 4 records re-delivered in order) |
| No out-of-order removal | fail-closed `OutOfOrderCommit` refusal removes nothing | `an_out_of_order_commit_is_refused_and_removes_nothing_in_the_pilot_matrix` |
| Corruption on disk detected | CRC-32 framing; a flipped payload byte ends the replay at the last good record, loss COUNTED | `a_flipped_payload_byte_is_detected_and_truncated_with_a_counted_loss` |
| Overflow never silent | audited drop-of-OLDEST: hard counter + latched flag + reconciliation identity | `the_overflow_policy_drops_oldest_with_a_counted_audited_loss` |
| The chain works over REAL drains | `compute_chain_anchor` over `FileAuditBuffer::drain` output | `the_chain_anchors_a_real_buffer_drain_and_survives_nothing_unsaid` |

## 4. Retention (ledger step 2) — what is and is NOT proven here

- The edge deliberately carries NO age-based audit drop: audit evidence
  retention is a deployment `RetentionPolicy` decision (P0.16 §4; the P6.18
  recorded decision D-2), and P9.10 owns the per-class §16.3 periods.
- What this wave proves is the integrity EVIDENCE side: an authority
  export (`AuditExportBundle`, format
  `flexcore-edge-audit-export-v1`) with the platform-retained anchor, and
  `verify_bundle_to_evidence` producing the `VerifiedAuditExport` verdict
  (the integrity evidence the P9.29 authority pack attaches) — with the
  honest tamper-EVIDENCE wording in the verdict text.
- What is NOT proven here: that a live retention job cannot delete audit
  evidence early (there is no retention job in the code base to test; the
  §16.3 enforcement is the P9.10 design plus the Phase 7+ store contracts).
  Recorded as an open human/Phase 7 item, not as a pass.

## 5. What the proofs WOULD establish and what they would NOT

**Would establish (once a human runs them):** on the committed sources,
every tamper class in the table is detected with its named error; the
outage test shows zero audit-event loss and zero duplication with in-order
upload and exactly-once commit; a flipped payload byte on disk is
truncated-and-counted with the pre-corruption records intact; the export
bundle verifies and carries the honest evidence wording.

**Would NOT establish:**

- Protection against a determined adversary with sustained write access to
  the edge host (no secret in the chain — the explicit residual risk above;
  compensating controls: OS-level site protection of the journal path per
  the P6.18/D6 deployment profile, platform-side anchor retention, and the
  HMAC replacement).
- That the PILOT's deployed store, upload channel, or retention jobs
  behave this way — the tests run against the crate, not the deployment
  (the live outage test is the P9.20 drill / P9.24 rehearsal).
- Complete tamper coverage beyond the finite matrix (§13.3: a test
  outcome, not a guarantee).
- Certification of any kind (AGENTS.md §3): nothing here is a safety case
  or a conformance claim.

## 6. Defects/gaps found in existing Phase 6 code (RECORDED, NOT FIXED)

| # | Finding | Where | Disposition |
|---|---|---|---|
| 1 | The audit journal stores records IN THE CLEAR (encryption at rest not implemented) | `crates/edge-agent/src/audit_buffer.rs` module header (already recorded there twice) | Stands as the recorded DEPLOYMENT REQUIREMENT; P9.09/P9.10 own the pilot posture; no code change this wave |
| 2 | The audited counters (`appended_total`, `dropped_oldest`, `corrupt_tail_truncated`, `has_overflowed`) are in-memory PER OPEN — a restart loses the counts (the loss events themselves were audited at the moment they happened) | `audit_buffer.rs` durability note (already recorded there) | Stands; persisting counters remains the recorded P6.17 candidate. The P9.32 chain + platform-retained anchor is the cross-restart integrity evidence that does not depend on those counters |
| 3 | No hash-chain/integrity evidence existed on the audit surface before this wave — the P6.18 CRC framing detects bit corruption but not a re-written journal (CRCs recomputed over altered records pass) | `audit_buffer.rs` framing | Closed ADDITIVELY by this wave's chain + anchor for EXPORTED record sets; the journal-on-disk itself remains protected by the OS-level controls, not by cryptography — recorded, not hidden |

No existing file's logic was edited.

## 7. Reviewer-verification procedure (human)

1. `cargo fmt --check` and `cargo clippy -- -D warnings` on
   `crates/edge-agent` (nothing installed or built by the agent).
2. `cargo test -p flexcore-edge-agent --test audit_integrity_proof_p9_32`
   — 9 tests; every one must pass; a `HeadMismatch`/`CountMismatch` in an
   "honest copy" assertion is a REAL finding.
3. Independently re-run one tamper case: edit one byte of a drained
   record's `detail` and confirm `verify_against_anchor` names
   `HeadMismatch` — then recompute the chain by hand understanding the
   fold (previous digest first) to confirm the detection is structural,
   not incidental.
4. Confirm the file set via `git status`: only the four files in the header
   table (plus the P9.31 sibling record); the `lib.rs` diff is two additive
   `pub mod` lines.
5. Confirm every "would NOT establish" statement above survives review —
   a reviewer who finds the record overclaiming should tighten the wording
   in THIS file, not in code comments alone.

## 8. Phase 7 follow-ups (contract gaps, never parallel contracts)

1. **Cryptographic chain digest** — HMAC-SHA-256 (or better) under the
   reviewed key-management scheme, upgrading the anchor from tamper-
   EVIDENCE toward tamper-resistance for the pilot record set (the
   standing deployment requirement; P9.09 owns the key inventory).
2. **Anchor upload contract** — the platform-side endpoint/record that
   receives and retains the `ChainAnchor` at export time (the platform
   half of the offline-isolation discipline; the edge half exists here).
3. **At-least-once duplicate consumption** — confirm the platform's
   P4.24-style idempotent ingest classifies re-delivered audit records
   (the crash-window path) as duplicates BY anchor+sequence, and that
   classification is itself audited.
4. **Retention enforcement** — the §16.3 per-class periods (P9.10) become
   a store contract with a deletion-audit trail; this wave provides the
   integrity evidence, not the enforcement.
