<!-- P9.27 — Known-limitations and residual-risk register. CONSOLIDATION RECORD: it TRACES and NUMBERS every open risk carried by the phase-9 records (01–21, 22–24, 35–40 — the complete record set) and CITES — never restates — the carried Phase 6/7/8 open-item registers (O-registers). The honest headline limitation: NO pilot exists and NO execution evidence of any kind exists. Every acceptance/likelihood/owner/expiry cell is EMPTY/UNSIGNED; the register is itself the phase's risk output and is versioned with a dated-amendment procedure. No certification or safety wording anywhere. -->
# P9.27 — Known-Limitations and Residual-Risk Register (Consolidation — v0.1-draft; every assessment cell EMPTY, nothing accepted)

Date: 2026-11-14. Subtask: P9.27 (`ledger_phase_9.md` §P9.27).
Owner role: Product (with security and power systems). Report trace:
§23 Stage 9; §17.4 (threat model); §22.3 (release gates); §0.3 (what is
deliberately not promised). Inputs: all prior Phase 9 subtask records;
P9.07 findings (template — `07_deployment_security_assessment.md`);
P9.03 model coverage limitations (`03_network_model_build.md`); P9.15
HIL dispositions (`15_hil_test_plan.md` — none exist yet).

## 1. Honest status — the plain statement, and the largest limitation first

**The largest limitation of the whole register is that NO pilot exists
and NO execution evidence of any kind exists yet.** No site, no
customer, no deployment, no build, no test run, no drill, no shadow
data, no HIL rig, no operator — nothing in Phase 9 has executed
anything (the Phase 1–8 standing caveat applies: all code is
WRITTEN-UNRUN, awaiting the human build/test pass). Every "risk" below
is therefore a risk recorded by a DESIGN/TEMPLATE record, not a risk
measured at an operating pilot; no likelihood in this register can
have an empirical basis until the pilot runs, and none is invented
here.

This register CONSOLIDATES: it traces and numbers every open risk the
phase-9 records carry to P9.27 (§3), and cites — never restates — the
carried Phase 6/7/8 open-item registers (§4). It does not re-litigate
any source record; the source record remains the description of
record, and the LR-id is the stable handle P9.28, P9.29, and the Phase
10 release notes (P10.26) must cite.

**Register version: `pilot-limitations-register-v0.1-draft`
(2026-11-14).** Like the Phase 8 benchmark artifacts, the register is
versioned: every change is a DATED AMENDMENT (never an in-place edit
that loses the prior version), because a limitation quietly softened
between two edits is the ledger's named failure mode. v0.1-draft is a
DRAFT: nothing in it is accepted, acknowledged, or closed.

## 2. Structure of an entry (and what is EMPTY)

Every traced entry (§3) has these fields:

| Field | State in v0.1-draft |
|---|---|
| LR-id | assigned (LR-001…LR-211) — stable handle |
| Source record path | assigned (the trace; the source keeps the full wording) |
| Description (traced, one line) | short trace of the source's own wording — the source row is authoritative |
| Impact if it materialises | `<EMPTY>` — a human/product assessment against the real deployment |
| Likelihood (with the BASIS for the assessment) | `<EMPTY>` — per the ledger's rule, no likelihood is recorded without a basis; no basis exists (§1) |
| Owner (a named person) | `<EMPTY>` — no person exists in this design |
| Mitigation or monitoring | `<EMPTY>` — filled when the pilot deployment exists; the source record's structural controls are cited in its own text |
| Expiry / review date | `<EMPTY>` |
| Customer acknowledgement (for items affecting customer sites) | `<EMPTY — UNSIGNED>` (§6) |

## 3. The consolidated register — phase-9 traces (LR-001…LR-211)

Records 22–24 (`22_market_operational_procedures.md`,
`23_incident_response.md`, `24_backup_dr_rehearsal.md`) were
concurrent-wave deliverables; this register's first draft recorded
them as PENDING rows, and the same-writing-pass amendment
(§3.28–§3.30 below) traces their risk tables once those records landed
— v0.1-draft
therefore consolidates the complete 01–21 + 22–24 + 31–41 record set.
Omitting them would be the "limitation omitted from the release notes"
failure mode in reverse — the pending gap was recorded first, so the
gap itself was never invisible.

### 3.1 From `01_pilot_scope_freeze.md` §6 (LR-001–LR-006)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-001 | 01 §6 r1 | No real pilot customer exists; the scope freeze cannot be signed |
| LR-002 | 01 §6 r2 | D1 (jurisdiction/market) OPEN — pilot scope rests on no recorded jurisdiction |
| LR-003 | 01 §6 r3 | D2 (first portfolio and service) OPEN — no decision behind the tick |
| LR-004 | 01 §6 r4 | D3 OPEN — network class is a RECOMMENDATION only |
| LR-005 | 01 §6 r5 | Stage 1–8 gates NOT HELD — the pilot qualifies a platform whose stage gates ran on recorded exceptions |
| LR-006 | 01 §6 r6 | Scope-creep exposure: site/asset/service addition without re-issuing the freeze |

### 3.2 From `02_site_asset_reconciliation.md` §7 (LR-007–LR-010)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-007 | 02 §7 r1 | Stale customer asset lists; unrecorded replacements/retrofits |
| LR-008 | 02 §7 r2 | Capability values from datasheets mismatching installed firmware |
| LR-009 | 02 §7 r3 | Registry schema vs site reality drift (PCC, grid-model reference absent pre-P9.03) |
| LR-010 | 02 §7 r4 | Command-store ID-resolution proof UNRUN — no command-store round trip ever executed |

### 3.3 From `03_network_model_build.md` §10 (LR-011–LR-016)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-011 | 03 §10 r1 | D3 OPEN — a wrong class invalidates the built model |
| LR-012 | 03 §10 r2 | Estimated impedances presented as measured |
| LR-013 | 03 §10 r3 | Unbalanced network modelled as balanced |
| LR-014 | 03 §10 r4 | Single-benign-point validation trusted at reverse-power-flow extremes |
| LR-015 | 03 §10 r5 | Grid worker NEVER RUN (UNVERIFIED-API) — first real build is first execution of the provider path |
| LR-016 | 03 §10 r6 | No DSO data received; no site exists to request it from |

### 3.4 From `04_customer_integration_design.md` §13 (LR-017–LR-023)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-017 | 04 §13 r1 | No pilot customer; no integration design can be produced or reviewed |
| LR-018 | 04 §13 r2 | D5 OPEN — adapter/protocol set rests on a RECOMMENDATION |
| LR-019 | 04 §13 r3 | D1 OPEN — late closure reopens the integration design |
| LR-020 | 04 §13 r4 | Vendor firmware/profile drift: documented profile ≠ installed version |
| LR-021 | 04 §13 r5 | Write capability enabled by default in a read-only-qualified adapter |
| LR-022 | 04 §13 r6 | Conformance suites never executed (P9.37 UNRUN) — no adapter may be called conformant |
| LR-023 | 04 §13 r7 | Reactive-power sign undeclared per device profile → wrong Q into feasibility |

### 3.5 From `05_site_network_design.md` §13 (LR-024–LR-030)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-024 | 05 §13 r1 | No real site network; design cannot be reviewed or approved |
| LR-025 | 05 §13 r2 | P9.04 is a template — protocol cells cannot be filled |
| LR-026 | 05 §13 r3 | "Temporary" inbound rule or undocumented vendor remote-access path at commissioning |
| LR-027 | 05 §13 r4 | Edge buffer sizing from unmeasured assumptions; long outage exhausts storage |
| LR-028 | 05 §13 r5 | Unidirectional-gateway pattern conflicts with any future command path |
| LR-029 | 05 §13 r6 | OpenEMS REST hop cleartext (`http://`) inside the OT boundary (P6.10/P6.11 open item) |
| LR-030 | 05 §13 r7 | Edge journal encryption at rest not implemented |

### 3.6 From `06_ot_it_boundary_d6.md` §10 (LR-031–LR-038)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-031 | 06 §10 r1 | D6 OPEN — no signed per-deployment closure; every read-only-pilot entry condition blocked |
| LR-032 | 06 §10 r2 | No real site/architecture to review; design-level verdicts could diverge from reality |
| LR-033 | 06 §10 r3 | OpenEMS REST hop cleartext inside the OT boundary |
| LR-034 | 06 §10 r4 | Signed-update path DESIGN ONLY — pilot must not run without the P9.34 control verified |
| LR-035 | 06 §10 r5 | Secure boot undetermined per profile; compensating control unexecuted |
| LR-036 | 06 §10 r6 | Edge journal encryption at rest unimplemented |
| LR-037 | 06 §10 r7 | Modified-edge-client lab acceptance (P6.18 H2) and app-role/RLS review (H3) UNRUN |
| LR-038 | 06 §10 r8 | Rev. 4 draft pressure may change baseline expectations mid-pilot |

### 3.7 From `07_deployment_security_assessment.md` §15 (LR-039–LR-046)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-039 | 07 §15 r1 | NO security assessment exists — P9.28's "no must-fix open" check unevaluable |
| LR-040 | 07 §15 r2 | Production crypto absent: command signature is a keyed checksum; digests are FNV-1a-64 test-grade |
| LR-041 | 07 §15 r3 | Encryption at rest unimplemented; OpenEMS REST hop cleartext |
| LR-042 | 07 §15 r4 | Plugin-compromise and SBOM controls have NO pilot testing subtask — UNTESTED by construction |
| LR-043 | 07 §15 r5 | Broker-duplication testing has no named execution — UNTESTED unless named |
| LR-044 | 07 §15 r6 | Secure boot undetermined; update verification WRITTEN-UNRUN |
| LR-045 | 07 §15 r7 | Rev. 4 draft pressure (tracked as draft input only) |
| LR-046 | 07 §15 r8 | Telemetry-broker and head-gap-sequence design positions must be consciously accepted, not inherited |

### 3.8 From `08_identity_access_approval.md` §12 (LR-047–LR-053)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-047 | 08 §12 r1 | No IdP integration exists (only DenyAllResolver/dev resolver) — identity enforcement is future composition |
| LR-048 | 08 §12 r2 | No step-up/MFA enforcement mechanism in code (gap G1) |
| LR-049 | 08 §12 r3 | Approver shares TenantAdmin with administrative rights; finer role needs a reviewed change |
| LR-050 | 08 §12 r4 | Qualification-before-approval is procedural (no code reads a qualification register) |
| LR-051 | 08 §12 r5 | Break-glass expiry/revocation is procedural (no automatic revocation wiring) |
| LR-052 | 08 §12 r6 | No pilot user, IdP, mapping, or negative test — every acceptance criterion unmet |
| LR-053 | 08 §12 r7 | Alarm acknowledgement impossible on the current authz surface (gap G3) |

### 3.9 From `09_secrets_certs_keys.md` §12 (LR-054–LR-061)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-054 | 09 §12 r1 | No CA, secrets manager, HSM, or certificates exist |
| LR-055 | 09 §12 r2 | Production crypto absent (checksum-grade signing, test-grade digests) |
| LR-056 | 09 §12 r3 | `private_key_reference` cannot express a `secret-store:` reference (P9.33 finding 1) |
| LR-057 | 09 §12 r4 | Certificate expiry monitoring has NO implementation |
| LR-058 | 09 §12 r5 | Revocation propagation unmeasured (nothing to measure) |
| LR-059 | 09 §12 r6 | Compromised-device-id re-enrollment policy undecided |
| LR-060 | 09 §12 r7 | Hardware-backed key custody undetermined |
| LR-061 | 09 §12 r8 | Vendor default credentials (OpenEMS `admin`/`admin`, `ws://`) must be affirmatively excluded |

### 3.10 From `10_data_residency_retention.md` §13 (LR-062–LR-068)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-062 | 10 §13 r1 | D1 OPEN — no jurisdiction, no residency constraint, no privacy regime, no retention basis |
| LR-063 | 10 §13 r2 | Market rule requiring longer retention than the contract allows, found after deletion |
| LR-064 | 10 §13 r3 | RLS relied on while the application runs as table owner |
| LR-065 | 10 §13 r4 | Telemetry retained indefinitely because a class never got a period |
| LR-066 | 10 §13 r5 | Edge telemetry journal encryption at rest is an OPEN item |
| LR-067 | 10 §13 r6 | Append-only audit evidence vs erasure obligations — resolution jurisdiction-dependent, EMPTY |
| LR-068 | 10 §13 r7 | Tenancy negative test, retention inspection, log inspection all UNRUN — open verification debt |

### 3.11 From `11_market_program_requirements.md` §11 (LR-069–LR-075)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-069 | 11 §11 r1 | D1 OPEN — pilot scope rests on no recorded market decision; no capture can begin |
| LR-070 | 11 §11 r2 | No market contact exists; even the "no participation" record is unfilled |
| LR-071 | 11 §11 r3 | Designing to a neighbouring region's program rules |
| LR-072 | 11 §11 r4 | Assuming Order 2222 implementation where it is not live |
| LR-073 | 11 §11 r5 | Baseline method chosen by FlexCore instead of the program |
| LR-074 | 11 §11 r6 | No market-records store — a captured settlement requirement exposes a capability gap late |
| LR-075 | 11 §11 r7 | PJM capacity-market date conflict between sources — unresolved verification task |

### 3.12 From `12_authority_engagement.md` §12 (LR-076–LR-083)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-076 | 12 §12 r1 | D1 OPEN — authorities and adopted interconnection standard unrecorded |
| LR-077 | 12 §12 r2 | No authority engaged; every engagement UNSCHEDULED on an UNSIGNED freeze |
| LR-078 | 12 §12 r3 | Authority engaged late, after the pilot schedule is fixed |
| LR-079 | 12 §12 r4 | Assuming a neighbouring jurisdiction's adoption of IEEE 1547-2018 applies locally |
| LR-080 | 12 §12 r5 | Required impact study discovered during field acceptance |
| LR-081 | 12 §12 r6 | IEEE 1547-2018 revision in ballot — filled settings record must be re-checked on publication |
| LR-082 | 12 §12 r7 | Approval assumed from silence (no objection ≠ approved) |
| LR-083 | 12 §12 r8 | Superseded-edition adoption making a 2018-based settings record wrong for the site |

### 3.13 From `13_commissioning_plan.md` §11 (LR-084–LR-092)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-084 | 13 §11 r1 | No site/customer/hardware — commissioning UNSCHEDULED until the freeze is signed |
| LR-085 | 13 §11 r2 | D6 OPEN — a site entering shadow without signed closure is the named failure mode (C8.3 blocks it) |
| LR-086 | 13 §11 r3 | Edge hardware platform undetermined (P6.34) — install procedure PENDING PILOT HARDWARE SPEC |
| LR-087 | 13 §11 r4 | Signed-update path DESIGN ONLY — C3.1 fails closed until P9.34 verification exists |
| LR-088 | 13 §11 r5 | OpenEMS REST hop cleartext — C2.3 fails unless closed or explicitly accepted in signed D6 |
| LR-089 | 13 §11 r6 | Vendor commissioning documentation absent — walkthrough criteria may not match the real platform |
| LR-090 | 13 §11 r7 | Safety rules assumed from another site |
| LR-091 | 13 §11 r8 | Failed step dispositioned as "accepted" without a named decision owner |
| LR-092 | 13 §11 r9 | Buffer bound from unmeasured assumptions — C2.5 verifies design==deployed only |

### 3.14 From `14_lab_integration_test.md` §10 (LR-093–LR-100)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-093 | 14 §10 r1 | No lab environment exists (E1–E9 absent) — Stage 6 re-verification cannot start |
| LR-094 | 14 §10 r2 | Stage 6 lab run UNRUN — the pilot re-verifies an exit gate never run once |
| LR-095 | 14 §10 r3 | No PKI — a lab run without real mTLS tests a degraded posture |
| LR-096 | 14 §10 r4 | Lab adapter/EMS version drift vs field version silently invalidates the record |
| LR-097 | 14 §10 r5 | Fault injection skipped as "covered by unit tests" |
| LR-098 | 14 §10 r6 | A lab-enabled command misread as evidence the field path works |
| LR-099 | 14 §10 r7 | P9.03–P9.05 are templates — the configuration under test is undefined |
| LR-100 | 14 §10 r8 | A1–A8 OpenEMS REST assumptions UNVERIFIED until a lab diff |

### 3.15 From `15_hil_test_plan.md` §11 (LR-101–LR-108)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-101 | 15 §11 r1 | No HIL rig, no lab/vendor access — blocked behind a nonexistent lab |
| LR-102 | 15 §11 r2 | HIL run before the cyber review passes |
| LR-103 | 15 §11 r3 | PHIL interface instability mistaken for equipment failure |
| LR-104 | 15 §11 r4 | Test passed on lab firmware differing from field firmware |
| LR-105 | 15 §11 r5 | Rig unvalidated or calibration lapsed — not HIL evidence |
| LR-106 | 15 §11 r6 | HIL evidence misread as equipment certification or real-time performance |
| LR-107 | 15 §11 r7 | Profile/seed reuse by name while parameters differ between stages |
| LR-108 | 15 §11 r8 | Pilot equipment/adapter set undefined — Part C applicability cells unset |

### 3.16 From `16_field_acceptance_plan.md` §12 (LR-109–LR-116)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-109 | 16 §12 R-1 | No site, no customer, no acceptance criteria — FAT unschedulable |
| LR-110 | 16 §12 R-2 | Numeric thresholds unregistered — no result judgeable pass/fail |
| LR-111 | 16 §12 R-3 | Coverage EMPTY because upstream evidence (P9.15/P9.40) does not exist |
| LR-112 | 16 §12 R-4 | Thresholds fitted to observed results |
| LR-113 | 16 §12 R-5 | Mid-run version change silently invalidating earlier cases |
| LR-114 | 16 §12 R-6 | Acceptance witnessed only by Neryva |
| LR-115 | 16 §12 R-7 | Waiver pressure — a customer-signed waiver records a REAL residual risk |
| LR-116 | 16 §12 R-8 | Operator-approved cases exercised before shadow acceptance |

### 3.17 From `17_field_acceptance_execution.md` §10 (LR-117–LR-124)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-117 | 17 §10 R-1 | No site, no commissioning, no shadow data — record unfillable |
| LR-118 | 17 §10 R-2 | Skipping shadow mode to save time |
| LR-119 | 17 §10 R-3 | Failed operator-approved command treated as a "test artifact", site left promoted |
| LR-120 | 17 §10 R-4 | Acceptance evidence recorded without the versions that produced it |
| LR-121 | 17 §10 R-5 | Shadow review perfunctory — signature without the required review content |
| LR-122 | 17 §10 R-6 | HIL pass over-read as a field result |
| LR-123 | 17 §10 R-7 | Cross-site/cross-scale over-read from a successful first-site FAT |
| LR-124 | 17 §10 R-8 | Promotion pressure before P9.26 exists |

### 3.18 From `18_telemetry_clock_verification.md` §12 (LR-125–LR-132)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-125 | 18 §12 R-1 | No site, no source inventory, no shadow data — record unfillable |
| LR-126 | 18 §12 R-2 | A source clock drifting beyond the expiry bound makes "fresh" commands stale |
| LR-127 | 18 §12 R-3 | ESTIMATED values treated as measured in a schedule |
| LR-128 | 18 §12 R-4 | A quality mapping that collapses SUSPECT into GOOD |
| LR-129 | 18 §12 R-5 | The DEFAULTS over-read as verified values (100 ms / 30 000 ms are decision-record defaults) |
| LR-130 | 18 §12 R-6 | Sync-source procurement/reachability unsigned — edge holds commands (correct) leaving an operational gap |
| LR-131 | 18 §12 R-7 | The P9.39 load-side clock row may never exist — CS-5 rests on steady-state evidence only |
| LR-132 | 18 §12 R-8 | Freshness limits fitted to observed delays (X-8 violation pressure) |

### 3.19 From `19_command_dry_run.md` §11 (LR-133–LR-138)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-133 | 19 §11 R-1 | A gate that passes everything in shadow because the validator points at a stale model |
| LR-134 | 19 §11 R-2 | Approval simulated by the same person who proposed |
| LR-135 | 19 §11 R-3 | An expired command treated as issuable |
| LR-136 | 19 §11 R-4 | Band B unexpressible in the pilot composition (no simulator transport) |
| LR-137 | 19 §11 R-5 | Shadow lifecycle correctness is not field delivery evidence |
| LR-138 | 19 §11 R-6 | No dry run, site, model, or approver exists — every acceptance criterion unmet |

### 3.20 From `20_fallback_drills.md` §10 (LR-139–LR-145)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-139 | 20 §10 R-1 | A universal zero-setpoint fallback applied to an asset for which it is unsafe |
| LR-140 | 20 §10 R-2 | A hold policy holding a stale setpoint after topology changed |
| LR-141 | 20 §10 R-3 | A drill that "passes" without a real injection |
| LR-142 | 20 §10 R-4 | No fallback policy, site, asset, or operator exists — criteria unmet |
| LR-143 | 20 §10 R-5 | Local-protection independence OBSERVED in drills, never guaranteed by FlexCore |
| LR-144 | 20 §10 R-6 | Audit-buffer overflow provably loses the OLDEST records — loss-window acceptability is a customer/authority decision |
| LR-145 | 20 §10 R-7 | Drill coverage of D-8/D-9 depends on the pilot composition actually containing a plugin host/gRPC worker |

### 3.21 From `21_operator_training.md` §8 (LR-146–LR-151)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-146 | 21 §8 R-1 | Training on a system that differs from the pilot build |
| LR-147 | 21 §8 R-2 | Qualification-before-approval is procedural (no code reads the register) |
| LR-148 | 21 §8 R-3 | An operator who can approve but cannot recognise a stale or estimated value |
| LR-149 | 21 §8 R-4 | Qualification recorded for a role the person does not hold |
| LR-150 | 21 §8 R-5 | Alarm acknowledgement impossible on the current authz surface — routed outside FlexCore |
| LR-151 | 21 §8 R-6 | No trainee, session, assessment, or qualification exists — criteria unmet |

### 3.22 From `35_safety_invariant_map.md` §8 (LR-152–LR-156)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-152 | 35 §8 R-P9.35-1 | The invariant-9 composition map-gap: plugin→feasibility-gate chain proven per-layer, never end-to-end (Phase 7 carried; Phase 8 did not close) |
| LR-153 | 35 §8 R-P9.35-2 | The ledger-step-2 pilot tests (wrong-site identity, superseded context) have no scheduled run |
| LR-154 | 35 §8 R-P9.35-3 | Invariant 10's pilot evidence is entirely human drill output — no code test can supply it |
| LR-155 | 35 §8 R-P9.35-4 | The pilot's cryptographic posture rests on integrity-checksum-grade primitives until replacements land |
| LR-156 | 35 §8 R-P9.35-5 | Head-gap sequence admission (monotonic, not dense) is the recorded design position until the Phase 7 follow-up decides |

### 3.23 From `36_forged_command_drill.md` §9 (LR-157–LR-162)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-157 | 36 §9 R-P9.36-1 | P9.23 does not exist yet — the drill is UNSCHEDULABLE; schedule slip propagates to P9.28 |
| LR-158 | 36 §9 R-P9.36-2 | The alert-path leg depends on P9.25-verified alerting infrastructure (not built) |
| LR-159 | 36 §9 R-P9.36-3 | The drill proves containment of the FIVE recorded attack cases only — outside the closed vocabulary is untested |
| LR-160 | 36 §9 R-P9.36-4 | The forged/replayed defence rests on checksum-grade primitives; the drill is re-run after any replacement |
| LR-161 | 36 §9 R-P9.36-5 | Site-specificity: a drill at one named site does not transfer to another |
| LR-162 | 36 §9 R-P9.36-6 | The invariant-9 composition map-gap remains OPEN under the same controls the drill exercises |

### 3.24 From `37_adapter_conformance_pilot.md` §11 (LR-163–LR-169)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-163 | 37 §11 r1 | Suites never executed — no adapter may be described as conformant for the whole pilot period |
| LR-164 | 37 §11 r2 | Suite run against a different adapter/firmware version than the pilot's |
| LR-165 | 37 §11 r3 | A suite "passed" by skipping failing cases without recording them |
| LR-166 | 37 §11 r4 | A1–A8 remain UNVERIFIED — every sign/unit/endpoint assumption unconfirmed |
| LR-167 | 37 §11 r5 | D5 OPEN — the adapter set (and suite set) rests on a RECOMMENDATION |
| LR-168 | 37 §11 r6 | No lab environment yet — "lab first, then pilot configuration" cannot start |
| LR-169 | 37 §11 r7 | Conformance wording drift — "certified"/"conformant" without a P9.37 citation |

### 3.25 From `38_pilot_shadow_replay.md` §11 (LR-170–LR-176)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-170 | 38 §11 r1 | No shadow data exists — the §24 step 10 proof cannot start |
| LR-171 | 38 §11 r2 | Replay reuses original outputs instead of recomputing (the void-replay failure mode) |
| LR-172 | 38 §11 r3 | Tolerance fitted after seeing a divergence — voids the record |
| LR-173 | 38 §11 r4 | A beyond-tolerance divergence with no assignable cause stays an open determinism finding |
| LR-174 | 38 §11 r5 | ScenarioRecord v3 refuses non-synthetic records — the replay CLI cannot execute a pilot replay until the seam exists |
| LR-175 | 38 §11 r6 | Version drift between original shadow run and replay masquerading as nondeterminism |
| LR-176 | 38 §11 r7 | "Passing" replay over-read as field-readiness or safety evidence |

### 3.26 From `39_performance_load_test.md` §10 (LR-177–LR-182)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-177 | 39 §10 R-1 | No numeric threshold pre-registered — no run result judgeable pass/fail |
| LR-178 | 39 §10 R-2 | The load profile is PLACEHOLDER until P9.01's values are copied |
| LR-179 | 39 §10 R-3 | Lab-rig numbers misread as pilot numbers |
| LR-180 | 39 §10 R-4 | Degraded-case coverage depends on the pilot composition; a silently-dropping composition is a FINDING |
| LR-181 | 39 §10 R-5 | Acknowledgement latency may be unexpressible in a read-only/shadow pilot |
| LR-182 | 39 §10 R-6 | Clock-sync under load interacts with command expiry — correct fail-closed behaviour with an operational cost |

### 3.27 From `40_hil_field_traceability.md` §7 (LR-183–LR-188)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-183 | 40 §7 R-1 | The map is EMPTY because both upstream plans are templates |
| LR-184 | 40 §7 R-2 | Profile/seed reuse is a requirement, not a fact — exceptions will not be total |
| LR-185 | 40 §7 R-3 | CHIL-only coverage may leave PHIL-requiring FAT cases unmapped — a signed acceptance records a REAL residual risk |
| LR-186 | 40 §7 R-4 | Version drift between HIL and field stages |
| LR-187 | 40 §7 R-5 | A back-fitted map (the ledger's named failure mode) |
| LR-188 | 40 §7 R-6 | HIL pass over-read — the strongest failure pressure on the traceability artifact |

### 3.28 From `22_market_operational_procedures.md` §14 (LR-189–LR-195)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-189 | 22 §14 R-P9.22-1 | D1 OPEN — even the no-participation disposition is unfilled |
| LR-190 | 22 §14 R-P9.22-2 | No market adapter / records store / commitments route — procedures reference seams that do not exist |
| LR-191 | 22 §14 R-P9.22-3 | A procedure written from the generic manual instead of the program's rules |
| LR-192 | 22 §14 R-P9.22-4 | An event signal with no defined duplicate handling (program event-identity field EMPTY) |
| LR-193 | 22 §14 R-P9.22-5 | Settlement evidence retained in a form the program does not accept |
| LR-194 | 22 §14 R-P9.22-6 | A program whose response expectations an operator-approved-at-most pilot cannot honestly serve |
| LR-195 | 22 §14 R-P9.22-7 | P9.21 training references these procedures while the drafts are INACTIVE — training on drafts |

### 3.29 From `23_incident_response.md` §12 (LR-196–LR-203)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-196 | 23 §12 R-P9.23-1 | No customer contact/contract/deadline — no incident class has a measurable notification path |
| LR-197 | 23 §12 R-P9.23-2 | The I-1/I-6 alert leg depends on P9.25-verified alerting (not built); detection rests on human inspection |
| LR-198 | 23 §12 R-P9.23-3 | The tabletop is UNHELD — P9.36 stays UNSCHEDULABLE; slip propagates to P9.28 |
| LR-199 | 23 §12 R-P9.23-4 | The edge audit buffer is a bounded loss window, not an archive — prompt-export acceptability is a customer/authority decision |
| LR-200 | 23 §12 R-P9.23-5 | An incident class with no detection source would only be found by the customer |
| LR-201 | 23 §12 R-P9.23-6 | Evidence destroyed by a well-meaning remediation (order-of-operations is people-enforced) |
| LR-202 | 23 §12 R-P9.23-7 | Break-glass expiry/revocation is procedural — an incident-time elevation could outlive its box |
| LR-203 | 23 §12 R-P9.23-8 | FNV-1a-64 audit chain and keyed-checksum signature are tamper-EVIDENCE-grade, not cryptographic proof |

### 3.30 From `24_backup_dr_rehearsal.md` §11 (LR-204–LR-211)

| LR-id | Source row | Traced risk (short) |
|---|---|---|
| LR-204 | 24 §11 R-P9.24-1 | A backup configuration that has never been restored — until filled, the plan IS that gap |
| LR-205 | 24 §11 R-P9.24-2 | RPO/RTO requirements cannot be set (no customer, D1 OPEN); a run would measure against nothing |
| LR-206 | 24 §11 R-P9.24-3 | An outbox state that replays already-issued commands after restore |
| LR-207 | 24 §11 R-P9.24-4 | A restore that succeeds with an OLDER schema than the running code |
| LR-208 | 24 §11 R-P9.24-5 | A central outage longer than the audit-buffer bound provably loses the OLDEST records |
| LR-209 | 24 §11 R-P9.24-6 | Backup residency/encryption/retention cells EMPTY while D1 is OPEN |
| LR-210 | 24 §11 R-P9.24-7 | JetStream backup posture undecided — an event-plane gap the plan measures only via the DB |
| LR-211 | 24 §11 R-P9.24-8 | Audit-chain integrity verification is FNV-1a-64/CRC-32 tamper-EVIDENCE; replacements force a re-rehearsal |

## 4. Carried registers from Phases 6/7/8 (cited, NOT restated)

The prior-phase open-item registers remain the authoritative wording
for their items; this register adds the pointer and the assessment
structure, nothing else. Every item below is still open as of
2026-11-14 unless the source marks it closed. Items the pilot touches
directly are marked →.

| Carried register | Location | Items (by their own IDs) | Pilot-relevant highlights (pointer only) |
|---|---|---|---|
| Phase 6 open items O-1…O-12 (O-12 CLOSED) | `ledger/phase-06-edge-protocol/30_retrospective_release_gate.md` | O-1…O-11 open | → O-1 D5 OPEN (feeds LR-018/LR-167); → O-2 D6 OPEN (feeds LR-031); → O-4 uplink wire format REFUSES; → O-5 TLS on the OpenEMS REST hop (feeds LR-029/LR-033/LR-088); → O-9 authz action ids interim gates; O-10 unbalanced modelling deferred (feeds LR-013) |
| Phase 7 open-items register O-1…O-16 | `ledger/phase-07-plugin-sdk/30_retrospective_release_gate.md` §2 | O-1…O-16 | → O-1 the workspace build/test pass UNRUN (the single verification gate — part of §1's no-evidence statement); → O-2 D1–D6 OPEN; → O-3 license verifications UNVERIFIED; → O-11 security findings tables EMPTY; → O-15 compromised-HOST posture transports unauthenticated; → O-16 carries Phase 6 O-4/O-5 forward |
| Phase 8 findings carried O-1…O-15 | `ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md` §4 | O-1…O-15 | → O-2 §19.4 thresholds not pre-registered (X-8 — feeds LR-110/LR-132/LR-177 and the P9.25 threshold placeholders); → O-5 security F-1 HIGH unguarded `torch.load` at activation; → O-8 CI jobs never run; → O-9/O-10 authz gaps; → O-12 framework DECISION-POINTs OPEN; → O-13 ANDES GPLv3 legal gate OPEN. The research-module limitations (O-1, O-3, O-4, O-14) are research-track items — relevant to the pilot only in that no research output is pilot-qualified, which §5 states as a limitation, not a promise |

Count: 11 open Phase 6 items, 16 Phase 7 items, 15 Phase 8 items —
42 carried pointers in addition to the 211 phase-9 entries. The
per-item impact/likelihood/owner fields for these stay in their source
registers; this register's §2 structure applies only when the pilot
must accept one of them as a pilot-period residual (e.g. the cleartext
REST hop LR-029/LR-033/LR-088, which P9.06's D6 record requires to be
either closed or explicitly accepted in the signed D6 record).

## 5. Known limitations of the PILOT itself (the §0.3 / report-trace statement)

Beyond the traced risks, the pilot — even when executed — does not
have, and no record in Phase 9 may claim for it:

| # | Limitation | Trace |
|---|---|---|
| L-1 | No execution evidence of any kind exists yet: nothing built, nothing run (§1) — the pilot's evidence base starts empty | This record §1; Phase 6/7/8 O-1 items |
| L-2 | Not promised: hard real-time or protection-grade control; primary frequency response or synthetic inertia; autonomous grid switching; universal AC-OPF global optimality; universal market qualification or Order 2222 compliance; certified protocol conformance by adapter; safe field dispatch without site-specific integration/commissioning/cyber review/HIL; GNN/MARL outputs safe by construction | Report §0.3 (verbatim basis); AGENTS.md §3 rules 2–4 |
| L-3 | Simulation-first: the 0.x line is simulation-only; field write authority begins at v1.0.0 with operator approval; bounded automation is post-1.0 | `26_control_scope_document.md` §2/§8; AGENTS.md §4 |
| L-4 | Research outputs are NOT pilot-qualified: no evaluation has run (Phase 8 all evaluation cells EMPTY by design); no research module is compared to baselines or independently validated | Phase 8 `30_retrospective_exit_gate.md` §3/§5 |
| L-5 | One-site evidence: everything the pilot would establish is site-specific — no cross-site, cross-scale, or fleet claim is available from it | LR-123; `17_field_acceptance_execution.md` §7 |
| L-6 | Supervisory orchestration only — FlexCore never performs protection, inner-loop control, or sub-cycle control, and its drills observe (never guarantee) local-protection independence | LR-143; AGENTS.md §3 rule 3 |

## 6. The risk-acceptance structure (who accepts what — ALL EMPTY/UNSIGNED)

Acceptance of any residual risk is a HUMAN, SIGNED decision; an
assessor's acceptance is not the customer's acceptance (the P9.07
rule). The acceptance matrix for this register:

| Acceptance | Who signs | What it covers | Status |
|---|---|---|---|
| Customer acknowledgement of the register (ledger step 4) | The customer (named in the signed P9.01 freeze) — for the items affecting THEIR sites | The register's entries marked site-affecting at amendment time | `<EMPTY — UNSIGNED>` (no customer exists) |
| Site-owner acceptance | The site owner, per site | The site-specific entries (D6 conditions, network, commissioning, drills) | `<EMPTY — UNSIGNED>` |
| Neryva product acceptance | Neryva product ownership | The platform-side entries (instrumentation gaps, authz gaps, release-gate exceptions) | `<EMPTY — UNSIGNED>` |
| Security acceptance | Security (per `08`/`07` reviewer roles) | The security residual set (crypto posture, untested threats) | `<EMPTY — UNSIGNED>` |
| Authority-conditioned items | The applicable authority, where its engagement record (P9.12) requires it | e.g. loss-window acceptability (LR-144) where an authority is party | `<EMPTY — UNSCHEDULED>` |

Rule: an item is ACCEPTED only when its row here is signed and dated;
unsigned rows remain OPEN no matter what any other record says.

## 7. Carry-forward and self-consistency

1. **The register is itself the phase's risk output.** This record is
   a template consolidation: it can consolidate honestly because the
   source records wrote their risk tables; it cannot assess, because
   assessment needs the pilot. Its own gap — the EMPTY assessment
   structure (§2) — is recorded in itself; the register does not hide
   its own incompleteness (§1 records it first, in the plain
   statement).
2. **P9.28** consumes this register in the acceptance pack; a site is
   not approved while a must-affecting row of this register is
   unsigned.
3. **P10.26 (release notes)** must carry every limitation recorded
   here — none may be omitted (ledger step 5); the release notes cite
   LR-ids or the register version, never paraphrase a limitation into
   softer language.
4. **Versioning**: changes to this register are dated amendments
   (§1). The current version `pilot-limitations-register-v0.1-draft`
   consolidates the complete record set: 211 traced phase-9 entries
   (§3/§4) plus 42 carried pointers (§4). Within this same writing
   pass, the first draft traced 188 entries with 3 PENDING-RECORD
   rows for the concurrent records 22–24; the same-pass amendment
   (§3.28–§3.30) traces those records' tables (+23 traced, −3
   pending), giving 211. Any further change states its count delta.

## 8. Reviewer-verification procedure (human)

1. Confirm §1's plain statement stands and no likelihood anywhere in
   this register carries a number or a basis (the ledger rule: no
   likelihood without a basis — none exists).
2. Spot-check traces: pick at least five LR-ids across different
   source records and confirm each summary matches the source record's
   own risk-table row, and that the source path is correct.
3. Confirm the COUNTS: 211 traced phase-9 entries (LR-001…LR-211,
   from records 01–21, 22–24, and 35–40); 11 + 16 + 15 carried
   pointers. Recount from the source records' risk tables; any
   mismatch is a register defect to fix by amendment.
4. Confirm records 22–24's risk tables are traced (§3.28–§3.30) and
   the §7 item-4 amendment history matches what happened (188 + 3
   pending → 211).
5. Confirm §6's acceptance matrix is wholly EMPTY/UNSIGNED and that no
   row claims acceptance by anyone.
6. Confirm no item is worded as a certification or safety claim
   (ledger acceptance (d)): no "certified", no "safe by construction",
   no protection-grade wording; §5's limitation list quotes §0.3 as
   exclusions, never as capabilities.
7. Confirm the register is versioned with the dated-amendment rule and
   that the ledger tick wording is "implemented, awaiting human
   build/test".

## 9. Filling commands (for the human executor)

- When the pilot deployment exists: fill the §2 assessment fields
  entry by entry FROM the source records' evidence (impact, likelihood
  WITH its stated basis, named owner, mitigation/monitoring, expiry),
  never from this register's one-line summaries.
- Obtain the §6 signatures at the customer review (ledger step 4);
  file the acknowledged register version as the P9.27 evidence for
  the CHECKLIST item "Limitations and risks registered".
- At P10.26: reconcile the release notes against this register's
  final version — every limitation present, every LR-id resolved or
  carried.

## 10. Open risks carried forward (the register's own contribution to Phase 10)

| Risk | Basis |
|---|---|
| The register itself is a DRAFT consolidation with EMPTY assessments — an acceptance decision taken on v0.1-draft would be uninformed by definition | §1, §6, §7 |
| The consolidation is one-line traces of the source records' own wording; a reader acting on an LR-id MUST read the source row — the trace is a pointer, never the full description | §2, §3 |
| The largest limitation stands: no pilot, no execution evidence — every entry is a design-stage risk record, not a measured one | §1 |

Status: **WRITTEN-UNRUN** — a register cannot be "run"; the customer
review, the assessments with bases, the named owners, and the
signatures are the human gates and have not happened and are not
scheduled. No pilot exists, no execution evidence of any kind exists,
and none is claimed.
