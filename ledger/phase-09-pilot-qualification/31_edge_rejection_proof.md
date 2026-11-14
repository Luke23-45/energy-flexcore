# P9.31 — Forged/replayed/expired/out-of-sequence rejection at the edge: PROOF SURFACE IMPLEMENTED (awaiting human build/test)

Date: 2026-11-14. Subtask: P9.31 (`ledger_phase_9.md` §P9.31). Owner role:
Security (with integration engineering). Report trace: §23 Stage 9; §17.4
(forged/replayed command); §18.2 (the edge validates every command); §14.3
(signature, sequence, expiry, idempotency). Inputs: the Phase 6 command-path
controls (P6.14 gate, P6.15 four-check verifier, P6.16 forwarder, P6.18
audit buffer); `ENTRY_NOTE.md` boundary 5 (this block is REAL code + tests
in `crates/edge-agent`, WRITTEN-UNRUN).

**Run-status honesty (AGENTS.md rule 7):** the code and tests in this wave
are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass
executes it. Nothing here claims a test passed, a build succeeded, or that
a live attack was injected at a real site — the LIVE attack drill is P9.36
(a human gate); this subtask delivers the code-level proof surface the
drill injects with.

Files (complete list; nothing outside it was touched):

| File | Status | Content |
|---|---|---|
| `crates/edge-agent/src/pilot_rejection_proof.rs` | NEW | The attack-case runner over the composed P6.15 verifier; the closed attack-case vocabulary; the fail-loud `ErroneouslyAccepted` outcome; the ADDITIVE §14.3 edge stand-in (`FileCommandIdBinding` journal) |
| `crates/edge-agent/tests/pilot_rejection_proof_p9_31.rs` | NEW, UNRUN | The 9-test attack matrix (forged/expired/replay/out-of-sequence/idempotency/zero-reach/fail-loud/durable-audit) |
| `crates/edge-agent/src/lib.rs` | ADDITIVE only | Two `pub mod` lines + comments; no existing line changed |
| `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | P9.31 checkbox tick only | Exact-match replacement, asserted count 1 before and after |

## 1. The proof discipline — the existing controls are PROVEN BY EXERCISE

Nothing in this wave re-implements a control. Every rejection in the proof
is produced by the EXISTING Phase 6 code, cited by path:

| Attack case | Named rejection (code) | Exact code path exercised |
|---|---|---|
| Forged (unsigned / malformed / wrong signer / valid-shape-wrong-digest — the forged-but-otherwise-valid payload) | `CommandVerificationRejection::Signature` (`signature`) | `crates/edge-agent/src/command_verify.rs` → `CommandVerifier::verify` step 1 → `check_signature` |
| Expired (valid-until elapsed against the INJECTED clock; degraded/unknown clock fails closed) | `CommandVerificationRejection::Expiry` (`expiry`) | `command_verify.rs` → `verify` step 2 (clock-quality fail-closed, then `valid_from <= now < valid_until`, then the authorization window) |
| Out-of-sequence (gap of 1 or N BEHIND the last accepted; unsequenced 0) | `CommandVerificationRejection::Sequence` (`sequence`) | `command_verify.rs` → `verify` step 3 (evaluated LAST; `FileSequenceCheckpoint` persistence) |
| Replayed (byte-identical resend of a captured accepted command; equal sequence) | `CommandVerificationRejection::Duplicate` (`duplicate`) | `command_verify.rs` → `verify` step 3 — `sequence == last` on channel `sequence_channel(asset, "command")` |
| Conflicting payload under an already-used idempotency id (ledger step 4) | `PilotRejectionProofError::ConflictingIdempotencyPayload` (audit code `idempotency_conflict`) | `crates/edge-agent/src/pilot_rejection_proof.rs` → `FileCommandIdBinding::bind` (the ADDITIVE control, below) |
| Reachability (ledger step 5) | — | The ONLY route from a gate-admitted command to an adapter is a `VerifiedCommand` through `command_forward.rs` → `CommandForwardingEngine::forward_verified` (its own separate allowlist + policy re-checks); a rejected verification produces NO `VerifiedCommand`, so nothing exists to forward |

The sequence check LAST is the recorded four-check discipline (the P4.28
`SequenceGuard` rule: a refused sequence never advances the window). The
tests pin it: after every forged/expired/replayed injection, the persisted
`FileSequenceCheckpoint` state is re-read and asserted UNCHANGED.

## 2. The one additive control — the §14.3 idempotency stand-in

**Recorded gap (found, not fixed — it lives in Phase 6 code this wave does
not own):** the P6.15 verifier's replay defence is SEQUENCE-based. The
command id is bound into the signature (so a conflicting payload under a
used id cannot pass check 1 unless RE-SIGNED by the real key), but the
verifier has no command-id memory: a legitimately re-signed command re-using
an already-accepted id with a DIFFERENT payload passes all four checks as a
fresh sequence. Ledger P9.31 step 4 requires exactly that case to be
refused. The platform-side §14.3 idempotency-key registry is the
authoritative control and does not exist yet in the code base (recorded
contract gap, Phase 7 follow-up below).

**The edge-side stand-in delivered here (ADDITIVE, new module only):**
`FileCommandIdBinding` — a file-backed command-id → canonical-binding
journal in the exact framing discipline of `FileSequenceCheckpoint` and
`FileAuditBuffer`:

```text
header:  b"FLEXCORE-EDGE-COMMAND-ID-BINDING" + format version byte (0x01)
record:  [u32 LE payload_len][u32 LE crc32(payload)][payload]
payload: [u16 LE id_len][id bytes][u32 LE binding_len][binding bytes]
```

- The canonical binding is the P6.15 `canonical_command_binding` string
  (reused by citation): every field that defines the command, in stable
  order, value as its exact bit pattern.
- A REBIND with a different payload is the named
  `ConflictingIdempotencyPayload` refusal — nothing is overwritten in
  place; the verifier is never reached; the conflict is audited
  (`EDGE_COMMAND_ID_BINDING_CONFLICT`).
- A same-payload rebind (a byte-identical replay arriving at the guard)
  PASSES the guard — the four-check verifier owns the replay verdict and
  rejects it as `duplicate`. The two controls compose; neither substitutes.
- Durability BEFORE state: the record is `sync_data`-flushed before the
  in-memory map moves — a crash cannot lose a binding, so the conflicting
  payload cannot slip through a crash window. Recovery is
  truncate-and-audit with a counted loss, never silent.
- Capacity is a DISTINCT-id bound; a full guard refuses NEW bindings
  FAIL-CLOSED (never drop-oldest: dropping a binding re-opens that id).
- Ordering honesty: the guard runs BEFORE the verifier in `run_attack` so
  no sequence advance is ever spent on a refused command — and it never
  bypasses the P6.14 gate (every proof command carries a real gate receipt).

## 3. The attack matrix (what the WRITTEN-UNRUN tests inject)

`crates/edge-agent/tests/pilot_rejection_proof_p9_31.rs`, 9 tests,
mirroring the Phase 6 integration-test style (injected `FixedClock`, real
gate receipts, real `FileSequenceCheckpoint` files, named-error matching):

| # | Test | Ledger step | Subtle cases covered |
|---|---|---|---|
| 1 | forged-but-otherwise-valid payloads → `signature` | 2 | unsigned; valid-format-wrong-digest; wrong signer; window NOT moved (checkpoint re-read) |
| 2 | exactly-at-expiry vs one millis before → `expiry` / fail-loud pass | 3 | the boundary is inclusive-expiry (`now >= valid_until`); the passing case is recorded as `ErroneouslyAccepted`, never coerced into a rejection |
| 3 | replayed captured command → `duplicate`, across an edge RESTART | 1 | new verifier + reopened `FileSequenceCheckpoint` + reopened binding journal; the post-restart replay is STILL `duplicate` |
| 4 | out-of-sequence gaps → `sequence` | 3 | gap of 1 behind, gap of N behind, equal → `duplicate` (distinct), sequence 0 → `sequence`; HEAD gaps (ahead of last accepted) are ADMITTED BY DESIGN — stated, not hidden |
| 5 | conflicting payload under a used id → `idempotency_conflict` | 4 | legitimately re-signed with a fresh sequence; binding NOT overwritten; byte-identical resend passes the guard and is a verifier `duplicate` |
| 6 | the binding guard survives a restart | 4 | reopened journal refuses the conflict in run 2 |
| 7 | ZERO adapter reach | 5 | counting forwarder over the P6.16 engine: 0 invocations across forged + expired + conflicting; exactly 1 invocation from a VerifiedCommand through `forward_verified` |
| 8 | mislabelled attack → fail-loud | — | a fresh valid command presented as "replay" is recorded `ErroneouslyAccepted` and audited — the proof never manufactures a rejection to close a case |
| 9 | every rejection durably audited | (P9.32 join) | composed with the P6.18 `FileAuditBuffer`: the rejection evidence survives an edge restart on disk |

## 4. What the proofs WOULD establish and what they would NOT

**Would establish (once a human runs them):** for the finite attack matrix
above, on the exact `crates/edge-agent` sources as committed, each injected
attack is rejected by the cited Phase 6 code with the named DISTINCT code,
each rejection is audited, no rejected command produced a `VerifiedCommand`
or reached a forwarder, and no refused command advanced the sequence window.

**Would NOT establish (§13.3 discipline — say it in the record, not just in
code):**

- A test suite is a FINITE test set. Zero observed rejections-missed is a
  TEST OUTCOME, not a proof that no injection outside the matrix can pass.
- The tests run against the CRATE, not against the pilot's deployed edge
  build. Ledger acceptance criterion (c) — "the tests run against the pilot
  edge build and versions" — is the P9.36 live drill / P9.14–P9.17 human
  execution, not this wave.
- The signature underneath is the P6.15 keyed-checksum scheme
  (`edge-sig:...:fnv1a64:<digest>`): tamper-EVIDENCE against casual
  tampering, NOT a cryptographic guarantee. Replacing it with a real MAC
  under reviewed key management remains the recorded P6.15 DEPLOYMENT
  REQUIREMENT (unchanged here; the `algorithm`-style versioned prefix makes
  the replacement a detectable format change).
- The idempotency binding journal is an EDGE-LOCAL stand-in. It does not
  give the cross-edge, platform-authoritative §14.3 idempotency registry
  the report names; a conflicting payload arriving at a DIFFERENT edge (or
  at the platform) is outside its scope.
- No adapter, no device, no site, and no live network is touched. "Zero
  attempts reach an adapter" here is proven at the code seam (a counting
  forwarder), not by inspecting a real adapter log — the log inspection is
  ledger step 5's human execution.

## 5. Defects/gaps found in existing Phase 6 code (RECORDED, NOT FIXED)

| # | Finding | Where | Disposition |
|---|---|---|---|
| 1 | No command-id memory: a legitimately re-signed command re-using an accepted id with a conflicting payload passes all four checks (fresh sequence) | `crates/edge-agent/src/command_verify.rs` (check 3 is sequence-only) | Edge-side stand-in delivered ADDITIVELY here (`FileCommandIdBinding`); the platform-side §14.3 idempotency registry is the Phase 7 follow-up |
| 2 | Head gaps ahead of the last accepted sequence are admitted (monotonicity, not density) — the ledger's "sequence number behind the last accepted" wording is satisfied, but a skipped-forward sequence is silently accepted | `command_verify.rs` check 3 | Recorded as the §14.3 monotonic-window design (the platform may skip sequences for superseded/cancelled commands); density enforcement, if wanted, is a Phase 7 contract decision, not an edge-side unilateral change |
| 3 | Rejected commands' ids are still BOUND in the idempotency journal (the guard runs before the verdict) | this wave's `pilot_rejection_proof.rs` | Accepted and documented: the binding records what payload the id presented; a later conflict under that id is still detected. Rebinding-only-on-acceptance would open a crash window the durability-first design forbids |

No existing file's logic was edited. No defect above was "fixed" by
editing another module.

## 6. Reviewer-verification procedure (human)

1. `cargo fmt --check` and `cargo clippy -- -D warnings` on
   `crates/edge-agent` (nothing installed or built by the agent).
2. `cargo test -p flexcore-edge-agent --test pilot_rejection_proof_p9_31`
   — 9 tests; every one must pass; any failure is a REAL finding (the
   proof is fail-loud by construction: an accepted attack prints
   `ErroneouslyAccepted` with an `EDGE_PILOT_ATTACK_ACCEPTED` audit line).
3. Re-run the matrix independently (ledger's verification method): pick at
   least test 1 (forged) and test 3 (replay-after-restart) and confirm the
   assertion targets match the cited Phase 6 functions above.
4. Confirm the file set: `git status` shows ONLY the four files in the
   header table (plus the P9.32 sibling record), and
   `crates/edge-agent/src/lib.rs` diff is the two additive `pub mod` lines.
5. Confirm no secret or key material is introduced: the fixtures' key
   values are lab strings; `CommandSigningKey` renders `<redacted>`.

## 7. Phase 7 follow-ups (contract gaps, never parallel contracts)

1. **Platform-side §14.3 idempotency-key registry** — the authoritative,
   cross-edge duplicate/conflicting-payload control; the edge journal here
   is the local stand-in. Owner: platform engineering (Phase 7 contract).
2. **Cryptographic signature replacement** — HMAC-SHA-256 or asymmetric
   under reviewed key management (the standing P6.15/P6.14 deployment
   requirement; P9.09 owns the key inventory for the pilot).
3. **Sequence-density decision** — whether head gaps must be refused or
   rate-limited per channel, or the monotonic-window design is confirmed
   for the pilot (recorded in the §17.5 invariant map, P9.35).
4. **Binding-journal rotation** — the DISTINCT-id capacity is deployment-
   sized; a rotation/compaction procedure (with the anchor discipline of
   P9.32) is a Phase 7 operations contract.
