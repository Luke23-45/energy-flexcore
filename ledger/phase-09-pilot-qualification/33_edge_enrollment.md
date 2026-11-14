# P9.33 — Edge enrollment ceremony and identity lifecycle: CEREMONY AS CODE (awaiting human build/test)

Date: 2026-11-14. Subtask: P9.33 (`ledger_phase_9.md` §P9.33). Owner role:
Security / platform engineering. Report trace: §23 Stage 9; §18.1 (secure
site enrollment and identity); §17.3 (mTLS, short-lived credentials); §18.3.
Inputs: the P6.06 enrollment seam (`crates/edge-agent/src/enrollment.rs`);
`ENTRY_NOTE.md` boundary 5 (REAL code + tests in `crates/edge-agent`,
WRITTEN-UNRUN).

**Run-status honesty (AGENTS.md rule 7):** the code and tests in this wave
are WRITTEN, NOT BUILT and NOT RUN. No `cargo` command of any kind was
executed. Every test below is UNRUN until the human build/test pass
executes it. THE CEREMONY EXECUTION ITSELF is a separate, bigger gate:
ledger step 4 ("test enrollment on one non-production edge end-to-end,
including a failed-enrollment case") is a HUMAN execution — nothing in this
wave enrolled an edge anywhere, and no code path in this module can: the
module is the ceremony STRUCTURE that gates and records an enrollment, and
`EnrollmentCeremony::execution_is_a_human_gate()` returns that statement as
a queryable fact.

Files (complete list; nothing outside it was touched):

| File | Status | Content |
|---|---|---|
| `crates/edge-agent/src/enrollment_ceremony.rs` | NEW | The closed ceremony state machine; the nonce challenge + ceremony-message replay protection (INJECTED clock); the single-use/short-lived token discipline; the no-self-approval approval gate; the compromise re-enrollment rule; the append-only `FileIdentityEventRegister` (CRC-32 framing, truncate-and-audit) |
| `crates/edge-agent/tests/enrollment_ceremony_p9_33.rs` | NEW, UNRUN | The 9-test ceremony matrix (happy path / illegal transitions / spent+expired tokens / wrong-site+wrong-identity / nonce-replay-expiry / no-self-approval / compromise lifecycle / register round-trip / durable audit) |
| `crates/edge-agent/src/lib.rs` | ADDITIVE only | One `pub mod` line + comment; no existing line changed |
| `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | P9.33 checkbox tick only | Exact-match replacement, asserted count 1 before and after |

## 1. The ceremony definition (ledger step 1) — issuer, lifetime, channel

| Question the ledger asks | The recorded answer (and where the code enforces it) |
|---|---|
| Who issues the enrollment token? | The PLATFORM IDENTITY AUTHORITY — the control-plane enrollment service behind `POST /api/v1/edge/enroll` (the P6.06 flow). The edge GENERATES nothing (`begin_request` takes the P6.06 `ProvisioningToken` but never reads its value; the token record carries a REFERENCE). Certificate issuance itself remains the recorded P6.06 DEPLOYMENT REQUIREMENT (no CA exists in the 0.x line) — unchanged |
| The token's lifetime? | SINGLE-USE and SHORT-LIVED: the ceremony refuses a token whose lifetime exceeds `DEFAULT_MAX_TOKEN_LIFETIME_MILLIS` = 15 minutes (15 × 60 000 ms), refuses an already-spent reference, and refuses an expired token against the INJECTED clock. 15 minutes is long enough for an install window and far too short to become a standing credential (the ledger's named risk) |
| The channel it travels on? | Out-of-band: a human delivers the token to the site at install time (it never rides a discovery protocol); the token → credential EXCHANGE rides the outbound-only mTLS enrollment flow of P6.06/P6.07. The ceremony travels on NO channel of its own — it is edge-local state plus the audit trail |
| First-boot trust? | The presented single-use token + the configured §16.2 tenant/site binding. Nothing else: no pre-shared key, no environment-derived trust, no self-signed identity |

## 2. The state machine (closed; every transition named; no self-approval path)

```text
Unenrolled --BeginRequest----------> Requested
Requested  --IssueChallenge--------> Challenged
Challenged --AnswerChallenge-------> Challenged (answered flag set)
Challenged --Approve---------------> Approved    [PLATFORM human/authority gate]
Approved   --Activate--------------> Active      [valid P6.06 credential record]
Requested|Challenged|Approved|Active --Revoke--> Revoked
Revoked    --ResetForReEnrollment--> Unenrolled  [fresh ticket; the revoked identity
                                                    can never be the authorizer]
```

Every OTHER edge is `EnrollmentCeremonyError::InvalidTransition` (`CEREMONY_TRANSITION_INVALID`)
with the from-state and the attempted transition named; a second revoke is
the distinct `CEREMONY_ALREADY_REVOKED` (the register is append-only, not a
status overwrite). The Approve gate:

- the ceremony only RECORDS a presented `CeremonyApproval` — it contains no
  code path that generates one;
- an approval whose actor IS the device is `CEREMONY_SELF_APPROVAL_REFUSED`;
- an approval with no authority reference, wrong device, or wrong scope is
  `CEREMONY_APPROVAL_INVALID`;
- the actual platform-side decision is a human/authority act on the control
  plane — recorded here, not performed here.

## 3. Spent/expired tokens (ledger step 2) and the challenge

- SPENT: the edge-local `TokenSingleUseLedger` refuses a re-presented
  reference with the FIRST-consumption instant. The platform-side
  consumption is the authoritative single-use enforcement (the P6.06 issuer
  contract); this ledger is the no-round-trip local guard — recorded, not
  substituted.
- EXPIRED: `expires_at <= now` (inclusive-expiry, the P6.15 boundary
  convention) against the INJECTED clock — never wall time.
- CHALLENGE: a nonce is derived from (device identity, token reference,
  binding, clock instant, entropy seed); `answer_challenge` refuses a wrong
  nonce, an expired challenge (TTL `DEFAULT_CHALLENGE_TTL_MILLIS` = 5
  minutes), and a REPLAYED ceremony message (the replay memory keys the
  message kind + nonce and never forgets within the ceremony). The message
  binding is the versioned string `ceremony-msg:v1|kind:...|device:...|nonce:...|at:...`
  — the replacement of the derivation scheme is a detectable format change.
- HONESTY: the nonce derivation is the test-grade FNV-1a-64 pattern (the
  P4.28/P6.14/P7.14 family) — an INTEGRITY CHECKSUM, not a security
  primitive. An un-guessable nonce needs a CSPRNG, which needs the crypto
  dependency this loop bans: PRODUCTION NONCE = DEPLOYMENT REQUIREMENT,
  recorded (the same honesty shape as the P6.15 signature and the P7.14
  digest).

## 4. Compromise re-enrollment (ledger step 3) — the old identity never vouches

1. `revoke(actor, reason)` appends `CEREMONY_REVOKED` to the register —
   append-only; the revoked term stays in the history next to the active
   term it ended (cross-reference: P9.09 owns the MEASURED revocation; this
   register is the evidence that measurement reads).
2. `begin_re_enrollment` refuses a `PriorIdentity` authorization basis with
   `CEREMONY_COMPROMISED_IDENTITY_VOUCHING_REFUSED` — the ledger's named
   risk ("re-enrollment that trusts the old (compromised) identity to vouch
   for the new one") is refused STRUCTURALLY, by the closed
   `ReEnrollmentAuthorization` vocabulary.
3. The legal basis is a fresh `PlatformAuthorityTicket` reference; the fresh
   TOKEN then goes through the NORMAL `begin_request` discipline (so the
   single-use, short-lived, issuer-named rules apply identically to
   re-enrollment — there is no second token path to maintain).
4. The full history survives a restart: the integration test asserts the
   register after both terms reads
   `Requested, Approved, Activated, Revoked, ReEnrollmentOpened, Requested,
   Approved, Activated` — append-only, in order.

## 5. What the WRITTEN-UNRUN tests would establish and what they would NOT

**Would establish (once a human runs them):** for the finite matrix in
`tests/enrollment_ceremony_p9_33.rs`, on the exact `crates/edge-agent`
sources as committed — every legal transition fires in the recorded order
and is audited under its dedicated event name; every illegal transition is
refused by name; spent and expired tokens are refused at the hand-computed
boundaries (inclusive-expiry; ceiling-equal accepted, ceiling+1 ms refused);
the wrong-site and wrong-identity credentials are refused with the ceremony
staying un-activated; wrong nonce, replayed message, and TTL-expired
challenge are refused; there is no self-approval path; the compromise
lifecycle keeps both terms in an append-only register that survives a
restart; a refusal composed with the P6.18 `FileAuditBuffer` survives an
edge restart on disk.

**Would NOT establish (§13.3 discipline — recorded, not just coded):**

- A test suite is a FINITE test set. Zero observed ceremony violations is a
  TEST OUTCOME, not a proof that no sequence outside the matrix can slip
  through.
- No enrollment was EXECUTED. Ledger acceptance criterion (d) — "the
  non-production enrollment test passed" — is ledger step 4's HUMAN
  execution on a real (non-production) edge, not this wave. Nothing here
  touches a platform, a network, or a real credential.
- No PKI exists (the P6.06 honesty record stands unchanged); the ceremony
  gates and records an exchange whose issuance side is still a DEPLOYMENT
  REQUIREMENT.
- The nonce derivation and every digest here are test-grade checksums, NOT
  security primitives; the CSPRNG replacement is recorded, not implemented.
- The single-use ledger is EDGE-LOCAL; the authoritative single-use
  consumption is the platform's. A token spent at the platform but never
  seen by this edge is invisible to this guard.
- The lifecycle evidence lives in an edge-local file. An adversary with
  host access can truncate it (recovery is truncate-and-AUDIT — the loss is
  counted, not prevented); the platform-side identity registry remains the
  authoritative lifecycle record (P9.09's inventory).

## 6. Defects/gaps found in existing code (RECORDED, NOT FIXED)

| # | Finding | Where | Disposition |
|---|---|---|---|
| 1 | `DeviceCredentialRecord.private_key_reference` is `Option<PathBuf>` — structurally a `file:`-scheme reference only. The P9.33 references-only rule names `env:`/`file:`/`secret-store:`; an edge whose key lives in a platform secret store cannot be recorded without abusing a path string | `crates/edge-agent/src/enrollment.rs` (P6.06) | Recorded contract gap; widening the reference type would EDIT an existing P6.06 type this wave does not own. Phase 7 follow-up (below) |
| 2 | `ScriptedCredentialIssuer` refuses re-enrollment of a REVOKED device id outright — consistent with the ceremony (a compromise requires a platform decision), but the PLATFORM POLICY the scripted behaviour implies (a compromised device id may never re-enroll; the hardware re-registers under a new id) is nowhere stated | `crates/edge-agent/src/enrollment.rs` (P6.06 test support) + P9.09's inventory | Recorded decision gap for P9.09's certificate/secrets inventory; not fixed here |
| 3 | The closed `EdgeAuditKind` vocabulary has no dedicated ceremony kinds (ceremony-requested/approved/activated/re-enrollment-opened) | `crates/edge-agent/src/ports.rs` | The recorded dedicated-name-leads-detail pattern (P6.14/P9.31/P9.32) is used; extending the closed vocabulary is a reviewed change outside this wave's ownership |
| 4 | No CSPRNG exists anywhere in the crate (no crypto dependency is permitted this loop) | module-level | The nonce derivation is loudly labelled test-grade; the production replacement is a DEPLOYMENT REQUIREMENT (§3 above) |

No existing file's logic was edited. No defect above was "fixed" by editing
another module.

## 7. Reviewer-verification procedure (human)

1. `cargo fmt --check` and `cargo clippy -- -D warnings` on
   `crates/edge-agent` (nothing installed or built by the agent).
2. `cargo test -p flexcore-edge-agent --test enrollment_ceremony_p9_33`
   — 9 tests; every one must pass; any failure is a REAL finding (the
   ceremony is fail-loud by construction: every refusal carries its named
   `CEREMONY_*` error and an `EDGE_ENROLLMENT_CEREMONY_REFUSED` audit line).
3. Also run the in-module unit tests
   (`cargo test -p flexcore-edge-agent lib::enrollment_ceremony`) — the
   known-answer tests for the closed state names, the canonical
   ceremony-message binding string (asserted character-for-character), the
   nonce determinism/input-sensitivity, and the FNV offset-basis genesis.
4. Confirm the file set: `git status` shows ONLY the four files in the
   header table (plus the P9.34 sibling record), and the
   `crates/edge-agent/src/lib.rs` diff is the ONE additive `pub mod` line
   (plus the P9.34 line).
5. Confirm no secret material is introduced: token values exist only inside
   `ProvisioningToken` (P6.06, redacted `Debug`/`Display`); the ceremony
   never calls `expose()`; the token record carries the REFERENCE only.

## 8. Phase 7 follow-ups (contract gaps, never parallel contracts)

1. **Widen `private_key_reference` to a scheme-prefixed reference type**
   (`env:`/`file:`/`secret-store:`) in the P6.06 credential record. Owner:
   platform engineering (Phase 7 contract).
2. **Platform-side ceremony API**: approval-ticket issuance/verification
   and the authoritative single-use token consumption contract (the edge
   ledger here is the local guard). Owner: control plane.
3. **CSPRNG ceremony nonce** under the deployment's reviewed crypto (the
   standing P6.15/P7.14-style DEPLOYMENT REQUIREMENT; P9.09 owns the key
   inventory).
4. **Identity-register rotation/compaction** (deployment-sized, with the
   anchor discipline of P9.32) — Phase 7 operations contract.
5. **Whether a compromised DEVICE ID may ever re-enroll** — a P9.09
   platform policy decision the scripted P6.06 behaviour implies but no
   record states (§6 finding 2).
