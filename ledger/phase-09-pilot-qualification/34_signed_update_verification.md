# P9.34 — Signed edge-update verification and rollback: VERIFICATION SURFACE IMPLEMENTED (awaiting human build/test)

Date: 2026-11-14. Subtask: P9.34 (`ledger_phase_9.md` §P9.34). Owner role:
Platform engineering. Report trace: §23 Stage 9; §18.3 (signed and verified
update bundles); §17.3 (secure boot/update where supported); §21.2. Inputs:
the P7.14 provenance pattern (`crates/flexcore-kernel/src/signing.rs`); the
Phase 6 signed-update POSITION
(`ledger/phase-06-edge-protocol/34_secure_boot_position.md`, §3 — DESIGN
ONLY until now); `ENTRY_NOTE.md` boundary 5 (REAL code + tests in
`crates/edge-agent`, WRITTEN-UNRUN).

**Run-status honesty (AGENTS.md rule 7):** the code and tests in this wave
are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass
executes it. **NO UPDATE HAS EVER BEEN BUILT, SIGNED, STAGED, OR APPLIED —
not by this wave, and not by any prior wave** (P6.20 was design-only).
Ledger steps 2 ("apply a valid update to one non-production edge") and 3
("an update that fails mid-apply … confirm rollback … the edge still
operational") are the recorded UNRUN HUMAN executions; this wave delivers
the verification/refusal/rollback-decision surface those executions drive.

Files (complete list; nothing outside it was touched):

| File | Status | Content |
|---|---|---|
| `crates/edge-agent/src/update_verification.rs` | NEW | The `UpdateManifest` (version, digest, signer key REFERENCE, minimum-version floor, approval reference); the fail-closed `UpdateGate` (verification BEFORE any write; `NoUpdateVerifier` default); the test-grade FNV-1a-64 verifier (loudly labelled; production hash = DEPLOYMENT REQUIREMENT); the append-only `FileUpdateLedger`; the activation/rollback decision rules incl. the never-silent downgrade |
| `crates/edge-agent/tests/update_verification_p9_34.rs` | NEW, UNRUN | The 10-test update matrix (valid path / bad-signature + untrusted-key before-any-write / fail-closed default / unsigned / floor boundary / rollback-with-reason / below-floor-override / impossible rollbacks / register round-trip / manifest validation) |
| `crates/edge-agent/src/lib.rs` | ADDITIVE only | One `pub mod` line + comment (plus the P9.33 sibling line); no existing line changed |
| `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | P9.34 checkbox tick only | Exact-match replacement, asserted count 1 before and after |

## 1. What this adds over the Phase 6 position (P6.20/P6.34)

The P6.34 secure-boot position recorded (§3, "Update" row) that the P6.20
bundle "is signature-verified BEFORE any swap" — as DESIGN ONLY: no
verification code existed anywhere. This wave is that code, at the seam the
edge owns:

| P6.34/P6.20 recorded intent | The code now behind it |
|---|---|
| "signature-verified BEFORE any swap" | `UpdateGate::verify_before_staging` — takes NO path, performs NO filesystem write; a refusal is a return value + an audit record. The register is appended only AFTER a verification verdict exists (activation) |
| "manifest version/checksums/expiry against the platform release key" | `UpdateManifest` (version, `bundle_digest_hex`, `digest_algorithm`, signer key REFERENCE, minimum-version floor, approval reference) + the trusted-signer-reference set; the untrusted-key refusal (`UPDATE_UNTRUSTED_SIGNER`) fires BEFORE the digest check (the recorded refusal order) |
| "automatic rollback on failed health re-activates a previously verified bundle" | `UpdateActivation::rollback` — re-activates a version the register has SEEN activated, with the recorded reason; audited `EDGE_UPDATE_ROLLBACK` |
| "a downgrade is an explicit operator-signed action" | The never-silent rule: below the deployment floor, a rollback is `UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED` unless a `RollbackOverride` (named actor + authority reference) is presented — and then the override rides BOTH the register record and the audit line |

## 2. The refusal order (recorded; the P7.14 order mirrored)

1. manifest PRESENT — else `UPDATE_UNSIGNED_BUNDLE` (an unsigned bundle is
   refused before any other check);
2. manifest VALID — else `UPDATE_MANIFEST_INVALID` (closed digest-algorithm
   vocabulary, lowercase-hex digest, scheme-prefixed signer reference with
   the VALUE never echoed, parseable version + floor, non-empty approval
   reference);
3. declared algorithm == the configured verifier — else
   `UPDATE_DIGEST_ALGORITHM_MISMATCH`;
4. signer reference in the trusted set — else `UPDATE_UNTRUSTED_SIGNER`
   (ledger step 1's untrusted-key case);
5. version >= the manifest's minimum-version floor — else
   `UPDATE_BELOW_MINIMUM_FLOOR`;
6. digest verdict `Matches` — else `UPDATE_DIGEST_MISMATCH` (ledger step
   1's bad-signature case) or `UPDATE_VERIFIER_REFUSED`.

The DEFAULT verifier is `NoUpdateVerifier` — refuses EVERYTHING (the
kernel `NoVerifier` posture: the absence of signing infrastructure can
never look like success). `TestOnlyFnv1a64UpdateVerifier` is the labelled
test-grade FNV-1a-64; **the production hash (SHA-256 or stronger, wired
through the `UpdateVerifier` port under reviewed key management) is a
DEPLOYMENT REQUIREMENT** — the same honesty shape as P6.15/P6.20/P7.14.
"Signature" here is honestly a digest-and-record scheme: tamper-EVIDENCE,
NOT a cryptographic guarantee.

## 3. The register and the never-silent downgrade

`FileUpdateLedger` is append-only (magic
`FLEXCORE-EDGE-UPDATE-REGISTER` + version byte; length-prefixed CRC-32
records; truncate-and-audit recovery with a counted loss; durable BEFORE
the in-memory view moves). The ACTIVE version and the previously-activated
set are DERIVED by replay — the register is the source of truth. The
rollback rules, in order:

1. the target must be a version this register has SEEN activated
   (`UPDATE_ROLLBACK_TARGET_NEVER_ACTIVATED`);
2. the target must be BEHIND the active version
   (`UPDATE_ROLLBACK_TARGET_NOT_BEHIND_ACTIVE` — a forward "rollback" is an
   upgrade wearing a rollback's clothes);
3. below the deployment's minimum-version floor: refused
   (`UPDATE_ROLLBACK_BELOW_FLOOR_REFUSED`) UNLESS an explicit
   `RollbackOverride` exists — and the override is never silent.

Versions are compared NUMERICALLY (`1.2.3 < 1.10.0`; lexical comparison is
refused by construction — a malformed core is
`UPDATE_VERSION_MALFORMED`), never lexically.

## 4. The pilot update policy (ledger step 4) — RECORDED RECOMMENDATION, NOT SIGNED

Ledger step 4 requires "the update policy for the pilot: who approves an
update, the staging order across sites, and the freeze window during
acceptance testing (P9.16/P9.17)". This is an ORGANIZATION decision — the
record below is the RECOMMENDATION with the approval gate explicitly
UNSIGNED (the ENTRY_NOTE boundary-1 discipline: the human gate signs, the
agent records):

| Policy element | Recorded recommendation | Status |
|---|---|---|
| Who approves an update | The pilot site operator + the platform security lead, recorded as the manifest's `approval_reference` (a named approval record on the platform side). The code enforces that SOME named approval reference exists, fail-closed; WHO signs it is the human decision | RECOMMENDATION — approval NOT SIGNED |
| Staging order | Lab → one non-production edge (the ledger step 2 execution) → the pilot site, ONE edge at a time, each activation visible in the site's register before the next edge proceeds | RECOMMENDATION — approval NOT SIGNED |
| Freeze window (P9.16/P9.17) | NO updates on any pilot edge from the start of acceptance testing until the acceptance review closes (P9.16/P9.17 own those gates; an update mid-acceptance would invalidate the evidence). The code contributes the register: an activation during the freeze would be VISIBLE as a register record — the freeze itself is an operational rule | RECOMMENDATION — approval NOT SIGNED |

## 5. The A/B slot decision (recorded, per the ledger)

Ledger P9.34 does NOT dictate a slot scheme. The recorded decision
(`UPDATE_SLOT_DECISION`, pinned by the tests): the update/rollback REGISTER
is the source of truth for the active version; a two-slot A/B layout (the
previous version retained untouched until the new one is verified and
activated) is the RECOMMENDED deployment shape — it is what makes the
rollback rule cheap (a re-activation, not a re-download). The code models
exactly what the ledger dictates: active version + verified candidate +
register.

## 6. Secure-boot position carried from Phase 6 (what this does and does NOT establish)

- This surface verifies the FlexCore update bundle ABOVE the host OS. It
  does NOT establish boot-chain integrity, and no claim of "secure boot"
  is made or implied — the P6.34 position stands unchanged: every
  per-profile hardware determination is **PENDING PILOT HARDWARE SPEC**.
- Where the pilot hardware's determination comes back "supported", the two
  controls are complementary — neither substitutes for the other (the
  P6.34 §3 table).
- Where it comes back "unsupported", THIS verification is the only
  FlexCore-layer integrity control, and the residual risk (an attacker
  below the FlexCore layer — bootloader/OS — replacing the agent wholesale)
  is the one the P6.34 record already states; it is not re-quantified here.
- This surface also does NOT protect the configuration: the rollback rule
  re-activates a previously-verified BUNDLE version; whether a rollback
  restores the configuration is a bundle-content decision (the ledger's
  named risk "a rollback that restores the binary but not the
  configuration") — the manifest schema has no configuration section
  DELIBERATELY (it is a version/digest/signer/floor/approval record); the
  bundle-content contract is the recorded Phase 7 gap below.

## 7. What the WRITTEN-UNRUN tests would establish and what they would NOT

**Would establish (once a human runs them):** for the finite matrix in
`tests/update_verification_p9_34.rs`, on the exact sources as committed —
bad-signature and untrusted-key bundles are refused with their stable tags
BEFORE any file is written (the scratch directory holds ZERO files after
the refusals; the register, opened afterwards, holds zero records); the
default gate refuses everything; the unsigned bundle is refused first; the
floor boundary is exactly at-equal-accepts/below-refuses (hand-computed);
a valid verified update activates and the register reports the new version;
a rollback re-activates a previously-activated version with the recorded
reason; a below-floor rollback is refused without an override and carries
the override in BOTH the register and the audit line with one; impossible
rollback targets are refused; the register survives a restart, refuses
foreign files, and truncates corrupt tails with a counted loss; manifest
validation refuses scheme-less key references WITHOUT echoing the value.

**Would NOT establish (§13.3 discipline — recorded, not just coded):**

- A test suite is a FINITE test set. Zero observed verification misses is a
  TEST OUTCOME, not a guarantee.
- No update was BUILT, SIGNED, STAGED, or APPLIED. Ledger acceptance
  criteria (b) and (c) — the valid update applying on a non-production
  edge and the mid-apply-failure rollback with the edge still operational —
  are the UNRUN HUMAN executions (steps 2 and 3); the health-check wiring
  that TRIGGERS an automatic rollback is the deployment's composition, not
  this surface.
- The digest is the test-grade FNV-1a-64 in the tests; a production
  SHA-256-or-stronger verifier is a DEPLOYMENT REQUIREMENT, not
  implemented. Tamper-EVIDENCE, not tamper-PROOF.
- No secure-boot claim (§6); no hardware anywhere was touched.
- The trusted-signer set in the tests is a lab string; the REAL set comes
  from P9.09's certificate inventory at deployment composition.
- The freeze-window and staging-order policy cells are RECOMMENDATIONS;
  they bind nobody until signed.

## 8. Defects/gaps found in existing code (RECORDED, NOT FIXED)

| # | Finding | Where | Disposition |
|---|---|---|---|
| 1 | NO update-bundle PRODUCER exists anywhere: P6.20's build/package procedure is design-only; no tool builds a bundle, no schema defines its content, nothing has ever been signed. The P9.34 surface verifies a manifest/bundle contract that has no upstream producer | repo-wide (P6.20 §5 design only) | The largest recorded gap; the verification side now exists and pins what the producer must emit. Phase 7 follow-up (below) |
| 2 | `DIGEST_ALGORITHMS` accepts the `sha-256` NAME while no SHA-256 implementation exists in the kernel (documented in that module's header); the same shape is mirrored here for the edge | `crates/flexcore-kernel/src/signing.rs` (P7.14) | Not a new defect — an inherited, honestly-labelled one; recorded for traceability so the edge record and the kernel record tell one story |
| 3 | The closed `EdgeAuditKind` vocabulary has no dedicated update kinds (verified/refused/activated/rollback) | `crates/edge-agent/src/ports.rs` | The dedicated-name-leads-detail pattern (P6.14/P9.31/P9.32/P9.33) is used; extending the closed vocabulary is a reviewed change outside this wave's ownership |
| 4 | The mid-apply failure path (ledger step 3's trigger) has no edge-side hook: the rollback DECISION rules are here, but nothing invokes `rollback` on a failed health check — that wiring belongs to the deployment composition (P6.17 health → update surface) and does not exist | composition gap | Recorded; a Phase 7 composition contract, not an edge-module change this wave could make additively |

No existing file's logic was edited. No defect above was "fixed" by editing
another module.

## 9. Reviewer-verification procedure (human)

1. `cargo fmt --check` and `cargo clippy -- -D warnings` on
   `crates/edge-agent` (nothing installed or built by the agent).
2. `cargo test -p flexcore-edge-agent --test update_verification_p9_34`
   — 10 tests; every one must pass; any failure is a REAL finding.
3. Also run the in-module unit test
   (`cargo test -p flexcore-edge-agent lib::update_verification`) — the
   hand-computed numeric version-ordering known answers (`1.2.3 < 1.10.0`;
   `2.0.0 > 1.9.9`; equality; malformed cores refused).
4. Confirm the file set: `git status` shows ONLY the four files in the
   header table (plus the P9.33 sibling record), and the
   `crates/edge-agent/src/lib.rs` diff is the additive `pub mod` lines only.
5. Confirm the before-any-write property independently: the refused-bundle
   test asserts an EMPTY scratch directory; spot-check that
   `UpdateGate::verify_before_staging` and `verify_inner` contain no `Path`
   parameter and no `std::fs` call.
6. Confirm no key material is introduced: the trusted-signer set is a lab
   string; rejected signer references are never echoed (the test asserts
   the Display of the refusal does not contain the presented value).

## 10. Phase 7 follow-ups (contract gaps, never parallel contracts)

1. **The bundle producer and content schema** — the tooling that builds a
   signed update bundle (and defines what "the bundle" contains, including
   the configuration-vs-binary question the ledger's rollback risk names).
   Owner: platform engineering (Phase 7 contract).
2. **Production SHA-256-or-stronger verifier wiring** through the
   `UpdateVerifier` port under reviewed key management (the standing
   P6.15/P6.20/P7.14 deployment requirement; P9.09 owns the key
   inventory).
3. **Health-check → rollback composition contract** — the edge-side
   wiring that invokes the rollback decision on a failed post-update
   health check (P6.17 health surface → this surface), with the
   automatic-vs-operator-approved boundary recorded per deployment.
4. **Update-policy sign-off** — the §4 recommendation table (approver,
   staging order, freeze window) signed by the pilot authority; the code
   enforces only the fail-closed approval-reference field.
