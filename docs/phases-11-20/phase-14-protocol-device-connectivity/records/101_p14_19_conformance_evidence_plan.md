# P14.19 — Adapter conformance evidence plan (human-gated certification paths)

| Field | Value |
|---|---|
| Subtask | P14.19 — certification paths recorded as HUMAN-GATED PROCEDURES (QualityLogic/CSIP test labs, OpenADR Alliance certification, DNP Users Group testing); the evidence each lab needs and what FlexCore can prepare in-repo; ZERO conformance claims [S-5][S-10][S-14] |
| Wave / agent | Phase 14 wave 3, Agent B (P14.19, P14.22–P14.24) |
| Deliverable class | **Plan record only.** No lab is contacted, no fee is paid, no certification path is begun, no application is filed, no test harness is acquired. The plan records what the HUMAN OWNER would do and what the repository can prepare as evidence structure — nothing more |
| Status | **ZERO CONFORMANCE CLAIMS.** No line of this record states or implies that FlexCore conforms to, is certified under, or is compliant with IEEE 2030.5, CSIP, OpenADR, DNP3/IEEE 1815.2, SunSpec, Sparkplug B, CIM/CGMES, IEC 104, or IEC 61850 (conventions §7.3; AGENTS.md §3 rule 2; the openems-adapter guide §6 claim boundary). Certification is a human-gated lab process forever |
| Sources | [S-5] QualityLogic Intro to 2030.5 (the gap-analysis source naming the 2030.5 test lab), [S-10] OpenADR Alliance (openadr.org — the certification body's own site, per the gap analysis), [S-14] DNP Users Group IEEE 1815.2-2025 release page; supporting context [S-1] IEEE 2030.5-2018, [S-3] SunSpec CSIP Implementation Guide v2.1, [S-15] MESA IEEE 1815.2 Guidelines. **None of the lab PROCESSES below is quoted from a primary process document** — every procedural detail (fees, durations, harness acquisition, submission mechanics) is UNVERIFIED and marked so |
| Files touched | This record only (plus the operator docs of P14.23, which cite it for the certification pointers) |

## 1. Why this is a plan and never a certification

CSIP/2030.5, OpenADR, and DNP3 conformance is certified by third-party test
labs and alliances against a RUNNING implementation — a process that
requires a built binary (the first human build is itself an open program
gate), a procured test harness, fees, signed agreements, and signed test
results. Agents cannot perform any of those acts, and this program forbids
claiming the outcome in advance (conventions §7.3; phase-14 context §3
constraint 5 and §5 boundary 1). What Phase 14 CAN honestly deliver is this
plan: which named paths exist per the gap-analysis register, what evidence
each lab would need from FlexCore, which of those evidence items the
repository already prepares (as written artifacts), and which items are
irreducibly human.

## 2. The three named certification paths (human-gated procedures)

Every row's procedure mechanics are UNVERIFIED readings of the named
source's general role (the lab exists / the alliance runs a program per
the gap-analysis register); the primary process documents must be
consulted before any human step. Each path is NOT-STARTED: no lab
contacted, no fee paid, no harness procured, no application filed.

| Path | Governing body / lab (per the gap-analysis register) | Standard surface FlexCore would test | Status |
|---|---|---|---|
| IEEE 2030.5 / CSIP | QualityLogic (the 2030.5 test lab named in [S-5]); CSIP test-lab context per the SunSpec CSIP Implementation Guide [S-3] | A running AggregatorServer speaking the CSIP-required resource tree over mTLS, tested against the lab's own harness and procedures [S-1][S-3][S-5] | **NOT-STARTED** — human-gated |
| OpenADR 3.0 | OpenADR Alliance certification program [S-10] | A running VTN (the record-21 endpoint design, built) tested under the Alliance's own process; 2.0b has a SEPARATE track (record 22) and is not this plan's path while P14-E-2 stands | **NOT-STARTED** — human-gated |
| DNP3 (IEEE 1815.2) | DNP Users Group testing [S-14]; MESA guidelines context [S-15] | A running outstation presenting the 1815.2-2025 DER profile to the Users Group's test procedures [S-14][S-15] | **NOT-STARTED** — human-gated |

**The remaining Phase 14 surfaces have NO certification path named
anywhere in the repository's source register.** SunSpec Modbus
conformance programs, Sparkplug B conformance (the Eclipse Tahu
community context [S-39] is a SECONDARY comparison, not a certification
authority), CIM/CGMES conformance (ENTSO-E's process context [S-17]),
IEC 104, and IEC 61850 have no in-repo named lab. This is recorded as
UNVERIFIED-ABSENCE (the same honesty form as the openems-adapter guide
§6: what the checked sources support, not a verified negative) — if a
program exists for any of them, the human owner names it, and this
record's evidence-preparation table (§4) applies unchanged.

## 3. The evidence each lab needs (what the human owner would bring)

| Evidence item | 2030.5/CSIP lab | OpenADR Alliance | DNP Users Group | Who can produce it |
|---|---|---|---|---|
| A running implementation of the adapter | the built AggregatorServer | the built VTN | the built outstation | **HUMAN** — the first workspace build is itself the open program gate (conventions §7.1); nothing in this program has ever compiled or run |
| The purchased standard text / test procedures | IEEE 2030.5-2018 + CSIP guide [S-1][S-3] | the OpenADR 3.0 spec [S-10] | IEEE 1815.2-2025 [S-14] | **HUMAN** — procurement; the texts are not in the repository |
| The lab's own harness + procedures | UNVERIFIED — the lab's process document governs | UNVERIFIED | UNVERIFIED | the LAB (acquired/contracted by the human owner) |
| Device/certificate infrastructure (2030.5-specific) | device CA chain, SFDI/LFDI derivation resolved, trust anchors per record 03 §3 | n/a | secure-transport posture resolved per record 24 §4 open questions | **HUMAN** — PKI operations (record 03 §4); Phase 13/18 surfaces |
| Fees, signed agreements, signed test results | all three paths | all three paths | all three paths | **HUMAN** — never an agent act |

## 4. What FlexCore can prepare in-repo (and already has, written)

The repository's contribution to each path is EVIDENCE STRUCTURE —
written artifacts that make a lab conversation possible. None of it is a
conformance result, and none of it executes:

| Prepared evidence | Where (written, UNRUN) | Path(s) it serves |
|---|---|---|
| The written-unrun CSIP use-case + primacy/lifecycle test suite | `crates/adapters/protocol-20305/tests/` (record 04) | 2030.5/CSIP — an in-repo skeleton for discussing coverage; the lab's procedures govern |
| The CSIP resource-tree model + addressing + scheduling design | `crates/adapters/protocol-20305/src/` (records 01/02) | 2030.5/CSIP |
| The 2030.5 security design + its UNVERIFIED check list | record 03 (§2–§3) | 2030.5/CSIP (the security-evidence items: chain validation, revocation behavior, SFDI/LFDI) |
| The OpenADR 3.0 endpoint/model/mapping design | `crates/adapters/protocol-openadr/` (record 21) | OpenADR |
| The OpenADR 2.0b assessment (the separate-track context) | record 22 | OpenADR (2.0b track, deferred) |
| The DNP3 DER-profile data-object vocabulary + outstation model + master-role deferral | `crates/adapters/protocol-dnp3/` (record 24) | DNP3 |
| The secure-transport posture note with its mandatory open questions | `crates/adapters/protocol-dnp3/src/secure_transport.rs` (record 24 §1) | DNP3 |
| The simulated counterparts (zero-hardware exercise of the adapters) | `crates/flexcore-simulator/` (P14.17, Agent A wave 3 — wait-checked at close) | all — simulator evidence is TEST-ONLY and is never evidence about real devices or labs |
| The per-adapter operator documentation (deployment/security/operations) | `docs/protocols/` (P14.23, record 103) | all — the deployment posture a lab engagement reviews |
| The per-adapter scope declarations + capability manifests | `crates/flexcore-protocol-ports/` + each adapter crate (records 01/21/23/24/43/44) | all — the declared-scope discipline every program reviews |

What the repository CANNOT prepare: the running binary, the wire-level
captures, the harness results, the signed conclusions. Those are rungs 2+
of the claim ladder (§5).

## 5. Claim-ladder semantics (the §13.3 discipline, Phase 12 record 51 form)

Phase 12's record 51 §2 fixes the program's claim-escalation ladder
(rung 0 written → 1 reviewed → 2 run once → 3 repeated; rung 3 is the
ceiling; no claim skips rungs). Applied to certification:

| Rung for conformance | What exists | Claimable form |
|---|---|---|
| 0 — written | this plan + the written-unrun suites/designs | "prepared evidence structure, written, never executed" — what Phase 14 has |
| 1 — reviewed | a static review traces the suites to the written adapter code | "traced to written code cited by record; the code is WRITTEN-UNRUN" |
| 2 — lab run once | a human-gated lab engagement runs the harness against a built adapter | "on `<date>`, the `<lab>` harness observed `<outcome>` against build `<id>` — recorded in the lab's signed result; a test outcome, never a guarantee" |
| 3 — repeated | re-certification cycles on later builds | the same form, per build — still never a guarantee |

**A lab pass is a test outcome about the tested build on the tested day
under the lab's procedures — never a guarantee, never a marketing claim,
and never a substitute for the alliance's own certification decision.**
Zero observed violations from a finite test set is a test outcome
(conventions §5, §13.3). Even a PASSED lab run entitles exactly the
openems-adapter guide §6 form of claim — and the OpenADR/SunSpec/DNP
bodies' own certification marks only after their own processes conclude.
FlexCore is not certified, and no text in this repository may say
otherwise.

## 6. NOT-STARTED / UNVERIFIED register

1. **NOT-STARTED** — every path in §2 and every HUMAN row in §3: no lab
   contacted, no fee paid, no harness procured, no standard text
   procured, no application filed, no date scheduled.
2. **UNVERIFIED** — every procedural detail (harness acquisition,
   submission mechanics, fees, durations, re-certification rules): the
   governing process documents were not consulted and are not in the
   repository. The gap-analysis register establishes only that the
   named bodies/labs exist as the program surfaces for these standards.
3. **UNVERIFIED-ABSENCE** — the absence of named certification programs
   for SunSpec Modbus, Sparkplug B, CIM/CGMES, IEC 104, IEC 61850 (§2
   closing paragraph): not a verified negative.
4. **UNRUN** — every artifact this plan points at (the suites, the
   adapter code, the simulators): written, never compiled, never
   executed. Simulator evidence (when P14.17's counterparts land) is
   TEST-ONLY forever.
5. **No conformance claim** — anywhere in this record or the artifacts
   it cites (conventions §7.3; the claim boundary of
   `docs/protocols/openems-adapter.md` §6 carries forward).

## 7. Handoff notes

1. **The human owner** — the decision queue, in order: (a) the first
   workspace build (the prerequisite for ANY lab conversation);
   (b) the standard-text procurement and verification passes named in
   the per-record UNVERIFIED registers (records 02 §4, 03 §3, 21 §5, 24
   §4, and siblings); (c) the P14-E-1 priority decision (record 102)
   naming the first program surfaces — which determines WHICH lab path
   to fund first; (d) lab selection, fees, and scheduling.
2. **P14.23 (this wave)** — the operator docs cite this record for the
   certification pointers per adapter; the docs themselves carry the
   same zero-claim boundary.
3. **Phase 19 (security/certification/compliance)** — this record is the
   Phase 14 input to that phase's certification planning; the §2 paths
   and §4 evidence inventory are consumed as records, not re-decided.
4. **Close record (P14.24)** — this record's zero-claim boundary is one
   of the honesty-sweep targets; the close audit re-checks it.
