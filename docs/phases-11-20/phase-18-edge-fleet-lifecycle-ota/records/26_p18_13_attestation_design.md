# P18.13 — Attestation design: boot-time evidence, measurement reporting, verification policy, and the honest hardware-absent fallback (P18-E-2)

| Field | Value |
|---|---|
| Subtask | P18.13 — Attestation design [S-166]; registers **P18-E-2 OPEN with DEFAULT** |
| Wave / agent | Phase 18 wave 1, Agent B |
| Status | **IMPLEMENTED, AWAITING HUMAN BUILD/TEST** (WRITTEN-UNRUN). Policy record + evidence types written; **no TPM/secure element is claimed, measured, or assumed; no attestation was ever performed** |
| Sources | [S-166] IETF T2TRG security-setup survey (gap analysis §11); P6.34 secure-boot position (`ledger/phase-06-edge-protocol/34_secure_boot_position.md`); P9.34 update seam (`crates/edge-agent/src/update_verification.rs` — consumed, never bypassed); consumers: P18.06 per-device attestation before authorization (Agent A), P18.14 negative tests (wave 2) |
| Files touched | `crates/edge-agent/src/attestation.rs` (new), `crates/edge-agent/src/lib.rs` (additive module declaration only) |

## 1. P18-E-2 — registered OPEN, DEFAULT (a) implemented as the record instructs

| Field | Value |
|---|---|
| ID | **P18-E-2** (phase-18 decision register; ledger §Decision points) |
| Question | Hardware trust anchors (TPM / secure element): what is assumed, and what changes without them? |
| Options | (a) hardware-optional design — attestation degrades to software evidence, the difference stated honestly per case; (b) hardware-required (REJECTED for 0.x — no fleet, no hardware spec); (c) ignore hardware (REJECTED — dishonest) |
| **DEFAULT (implemented per the subtask instruction)** | **(a)** — the assumption set is recorded (PENDING PILOT HARDWARE SPEC per the P6.34 posture); every attestation statement says what the platform checks WITH and WITHOUT an anchor |
| Status | **OPEN** — agents never close an E-decision (conventions §4) |
| What would close it | The pilot hardware spec (P6.34 cells) filling the determination |
| Closer | human owner |

The assumption set, verbatim from the pinned code statement
(`HARDWARE_POSTURE_STATEMENT`): *"every hardware determination is PENDING
PILOT HARDWARE SPEC (the P6.34 posture; P18-E-2 DEFAULT (a)
hardware-optional). A claimed anchor is a device REPORT this platform does
not verify; software-only evidence is consistency-checked self-attestation
that cannot detect a compromised reporter. No root of trust is assumed,
claimed, or implied anywhere in this module."*

## 2. The verification policy — what the platform checks before authorizing an update

`check_before_update_authorization` is a PURE function over test doubles
(the INJECTED verification instant is a parameter; no wall time), fail-closed
at every step; the FIRST failed check is the decision:

| # | Check | Refusal (named) |
|---|---|---|
| 1 | Evidence exists (and passes structural validation: identity, report reference, positive instant, non-empty schemes, no duplicate components) | `ATTESTATION_REFUSE_NO_EVIDENCE` |
| 2 | Device not revoked (P18.08 propagation consumed at this gate) | `ATTESTATION_REFUSE_REVOKED_DEVICE` |
| 3 | Evidence names THIS device | `ATTESTATION_REFUSE_EVIDENCE_DEVICE_MISMATCH` |
| 4 | Freshness: `reported_at` within the policy's age ceiling against the injected instant; a future-dated report is refused rather than negated into fake freshness | `ATTESTATION_REFUSE_EVIDENCE_STALE` |
| 5 | Counter monotonicity: reported boot counter >= last-seen (a regression is the signature of a restored image — exactly what an update authorization must not ride) | `ATTESTATION_REFUSE_COUNTER_REGRESSION` |
| 6 | Measurement check mode (below) | `ATTESTATION_REFUSE_MEASUREMENT_MISMATCH` |
| 7 | Anchor requirement (only if the policy sets `require_hardware_anchor`; the DEFAULT does NOT) | `ATTESTATION_REFUSE_HARDWARE_ANCHOR_REQUIRED` |

All-pass decision: `ATTESTATION_AUTHORIZE_UPDATE` — one gate among several;
the P18.06 orchestrator still applies cohort state and the P18.15
command-lifecycle blocking, and the P9.34 verification-before-any-write
surface is untouched (attestation authorizes WHETHER an update may proceed;
it never substitutes manifest verification).

Measurement check modes (closed): `NO_MEASUREMENT_CHECK` (records that NO
assertion is made — honest by construction), `BASELINE_DIGEST_EQUALITY`
(every expected component present and digest-equal; an unexpected or missing
component is a mismatch), `REFERENCE_REGISTER_MATCH` (scheme + digest per
component against the deployment's reference register — a digest-scheme
change is a detectable format change, never a silent pass). All policy
values are per-deployment configuration, pre-registered (X-8) before any
run that could fill them; the evidence-age default (24 h) is a recorded
recommendation, not a sourced figure.

## 3. WITH hardware vs WITHOUT hardware (the honest table P18-E-2 requires)

| | WITH a hardware anchor (future deployment; PENDING PILOT HARDWARE SPEC) | WITHOUT an anchor (the only case that can exist today) |
|---|---|---|
| Evidence shape | `AttestationEvidence` with `TrustAnchorMode::HardwareAnchorClaimed`; MAY carry anchor-signed measurements | `TrustAnchorMode::SoftwareOnlyEvidence` |
| What the platform verifies | **NOTHING is verified by this platform** — verifying an anchor signature is root-of-trust work requiring deployed hardware + a verifier service; NEITHER EXISTS. The module defines the evidence SHAPE such a deployment would report | Consistency only: freshness, counter monotonicity, measurement equality against recorded baselines |
| Assurance bound | Undefined until the hardware spec and a real verifier exist — no claim is made | Cannot detect a compromised reporter; the gate is a plausibility check, never a root of trust. Stated verbatim in the code statement and in this record |
| Policy effect | A deployment MAY set `require_hardware_anchor = true` once a verified anchor posture exists — a recorded policy decision, never silent | DEFAULT policy keeps it `false` (requiring an anchor today would refuse every device that can exist) |

Digest grade: the measurement digests CARRIED by the evidence type are
opaque text; this module computes no digest and verifies no signature.
Test-grade digest computation (the FNV-1a-64 family) stays loudly labeled
test-grade in the P18.14 fixtures; production measurement digests (TPM PCRs
where hardware exists, or a measured-boot software scheme where it does
not) are a DEPLOYMENT REQUIREMENT and a human-gated choice — PENDING PILOT
HARDWARE SPEC.

## 4. Fleet-journal integration (P18.12)

The attestation state rides the migration-0023 `ATTESTATION_STATE` rows
(`crate::fleet_registry` delegates the value domain here):
`NOT_REPORTED` (never healthy-by-default), `CURRENT`, `STALE`, `FAILED`,
`REVOKED`. Unknown strings are the named `FLEET_STATE_VALUE_UNKNOWN`
refusal.

## 5. Compliance mapping (mapping, never a certification claim)

| Deliverable aspect | Mapped requirement | Basis |
|---|---|---|
| Per-device attestation before update authorization | IEC 62443-4-2 CR 3.4 (as cited by the phase-18 plan) + CR 2.1 [S-163]; IEC 62443-4-1 SAR 3.4 (process) [S-164] | As cited by the phase-18 plan from the gap analysis §11 register. Clause-content interpretation **UNVERIFIED** — the mapping records where evidence lives, never conformance |

## 6. Honesty register

1. **WRITTEN-UNRUN** — never built, never executed; the in-module tests are
   complete and UNRUN (human gate).
2. **No fleet** — no evidence was ever collected, reported, or verified; no
   attestation decision was ever made against a real device; the gate is a
   pure function over test doubles.
3. **PENDING PILOT HARDWARE SPEC** — every hardware determination
   (P18-E-2; the P6.34 posture). No TPM/secure element is claimed,
   measured, or assumed.
4. **P18-E-2 stays OPEN** — this record implements the DEFAULT only; the
   closer is the human owner (the pilot hardware spec).
5. **Handoff** — Agent A's P18.06 consumes
   `check_before_update_authorization` as one gate among its own; wave 2's
   P18.14 consumes the named refusals for its negative tests.
