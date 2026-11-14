<!-- P10.16 — Deployment-specific cyber risk review (CONSOLIDATION). TRACE-ONLY
     record per ENTRY_NOTE.md boundary 5: it CITES the carried registers
     (Phase 6 O-items, Phase 7 §2 O-items, Phase 8 §4 O-1..O-15 + the P8.25 /
     P8.36 records, Phase 9 security design set 06/07/08/09/35/36 + the P9.27
     register) BY PATH and NEVER restates, re-decides, or CLOSES any item.
     The deployment-specific framing (site context, hardware, contracts,
     deployed network/IdP state) is ALL EMPTY. No release candidate exists
     (P10.18 UNRUN), so the candidate drift re-check is a PREPARED procedure
     with EMPTY verdict cells. NIST SP 800-82 Rev. 3 remains the recorded
     baseline; the Rev. 4 final re-check is carried as a P10.27 condition.
     D6 is OPEN and UNSIGNED. Nothing is closed here; every acceptance cell
     stays UNSIGNED. No assessment was executed — WRITTEN-UNRUN. -->
# P10.16 — Deployment-Specific Cyber Risk Review (Consolidation — trace-only; closes nothing)

Date: 2026-11-14. Subtask: P10.16 (`ledger_phase_10.md` §P10.16).
Owner role: Security. Report trace: §22.3 (release gate 12, cyber half);
§17; §25.2 (D6).
Inputs: P10.01 scope freeze
(`ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md` —
candidate identifier `<PENDING P10.18>`); P9.06 D6 review record
(`ledger/phase-09-pilot-qualification/06_ot_it_boundary_d6.md` — REVIEW
INPUT, NOT CLOSURE); P9.07 security assessment
(`ledger/phase-09-pilot-qualification/07_deployment_security_assessment.md`
— TEMPLATE, no assessment executed); P10.18 candidate build
(`ledger/phase-10-v1-release-hardening/18_ci_gate_run_record.md` — UNRUN,
no artifact exists); the NIST SP 800-82 Rev. 4 status
(`ledger/phase-10-v1-release-hardening/RESEARCH.md` §1).

## 1. Honest status — the plain statement

**This consolidation CLOSES NOTHING and assesses nothing.** It traces the
still-open cyber-risk items carried by the four phase registers and the
Phase 9 security design set into one deployment-facing table (§7), cites
each by path, and leaves every item open with its source as the record of
description. Per `ENTRY_NOTE.md` boundary 5, the source register remains
authoritative for its items; the LR-ids and O-ids below are pointers,
never restatements.

What this review CANNOT deliver, and does not claim:

- **No release candidate exists** (P10.18 is UNRUN; the candidate is a
  described build procedure, never a built artifact). The §17.4
  threat-control re-check against "the candidate configuration" therefore
  CANNOT run — the drift-check procedure (§3) is prepared, and every
  drift/verdict cell is `<EMPTY>`.
- **No deployment exists** to be deployment-specific about. The framing
  cells that make a cyber review deployment-specific (§2) are ALL EMPTY.
- **No assessment verdict is available**: the P9.07 assessment is a
  TEMPLATE with every status/evidence/severity/likelihood/residual cell
  EMPTY (`07_deployment_security_assessment.md` §1, §9, §10, §12; traced
  as LR-039 in the P9.27 register). Its status is carried, not upgraded.
- **Nothing is closed**: not D6, not a finding, not a baseline question,
  not an open-item register row. Every acceptance that appears below is
  `<UNSIGNED>`; the §22.3 "no must-fix finding open" check remains
  UNEVALUABLE (LR-039).

Ticking P10.16 means "implemented, awaiting human build/test/execution" —
the consolidation record exists; the review it consolidates toward is the
human gate.

## 2. What a REAL deployment-specific review adds — and why every cell is EMPTY

A product-side cyber review reads the design; a DEPLOYMENT-specific review
inspects the deployed thing. The ledger's P10.16 step 1 names that
difference ("re-check each §17.4 threat control against the candidate
configuration (not the pilot build) and record any drift"). What the
deployment-specific layer adds, and its state here:

| # | What a real deployment review adds | State in this consolidation |
|---|---|---|
| D-1 | Site context: the named site, customer OT architecture, zones, firewall/allowlist state as deployed (P9.05 §5/§6 inspection inputs) | `<EMPTY — no site, no customer, no deployed network>` |
| D-2 | Hardware: the edge platform, secure-boot state, TPM/HSM presence, gateway sizing (P6.34 determinations were PENDING PILOT HARDWARE SPEC) | `<EMPTY — no hardware spec exists>` |
| D-3 | Contracts: the customer contract, notification deadlines, incident-response obligations, data-processing terms (P9.23/P9.10 depend on them) | `<EMPTY — no contract exists>` |
| D-4 | Deployed identity state: IdP wiring, real users, role mappings, MFA enforcement (P9.08; the code ships `DenyAllResolver` + a dev resolver only) | `<EMPTY — no IdP, no users>` |
| D-5 | Deployed versions: control plane, edge agent, adapter/EMS versions as INSTALLED (P10.01 §5 — fleet versions, never datasheet values) | `<EMPTY — nothing installed>` |
| D-6 | The candidate configuration the re-check runs against | `<PENDING P10.18 — no candidate build exists>` |

The deployment-specific review is the human gate; this record prepares its
inputs and carries its obligations.

## 3. The §17.4 threat-control re-check against the candidate — PREPARED, UNRUN

Ledger step 1. The ten §17.4 threats and their design controls are cited —
with their testing subtasks and their EMPTY evidence/status/rating cells —
in `ledger/phase-09-pilot-qualification/07_deployment_security_assessment.md`
§9 (rows T1–T10). That table is the control inventory; this record does not
restate it. What P10.16 adds is the CANDIDATE re-check:

| Step | Procedure | Record cell |
|---|---|---|
| RC-1 | Fill the candidate identity from the P10.18 record (commit, tag, build ID, artifact hashes) into P10.01 §6.1 | `<PENDING P10.18>` |
| RC-2 | Walk `07_deployment_security_assessment.md` §9 row by row against the CANDIDATE configuration (the built artifacts, the composed defaults, the release configuration) — not the pilot build, not the design | Drift verdict per threat: `<EMPTY — UNRUN>` |
| RC-3 | For each row, name the testing subtask whose execution evidence must exist for the control to be more than UNTESTED (the P9.07 §9 rows already name them); confirm the evidence exists | Evidence reference per row: `<EMPTY — UNRUN>` |
| RC-4 | Record any drift (new dependency, changed default, changed composition) as a FINDING with owner + due state; a drifted control is re-assessed, never inherited | Findings: `<EMPTY — UNRUN>` |

Named risk carried (the ledger's own failure mode): "the review
consolidating the pilot build's evidence while the candidate drifted". No
drift is recorded here because no candidate exists to drift from — the
absence of a drift finding is NOT a clean-drift result.

## 4. Must-fix findings closure (ledger step 2) — UNEVALUABLE, nothing signed

- The P9.07 findings register (`07_deployment_security_assessment.md` §12)
  is `<EMPTY>`: no assessment has run, so no finding — must-fix or
  accepted-residual — has ever been recorded. The P10.16 check "every
  must-fix finding is closed with evidence, or has a signed, time-bounded
  acceptance" is therefore **UNEVALUABLE, and acceptance criterion (b) is
  NOT met by this record** — there are zero recorded findings and zero
  claimable closures (LR-039; the P9.07 §15 open-risk table).
- The findings that DO exist in the carried registers are cited in §7
  below (e.g. P8.25 F-1 HIGH, the P9.09/P9.35 crypto-posture items). None
  carries a signed acceptance; the acceptance route — signed, time-bounded,
  carried into P10.26/P10.27 per the ledger step — is the recorded
  procedure and nothing more. Per the P9.27 §6 rule, an item is ACCEPTED
  only when its row is signed and dated; unsigned rows remain OPEN no
  matter what any other record says.

## 5. NIST SP 800-82 Rev. 4 status (ledger step 3) — Rev. 3 remains the baseline; re-check carried as a P10.27 condition

| Item | Recorded state |
|---|---|
| Baseline | **NIST SP 800-82 Rev. 3** (final, September 2023) — the assessment reference framework (`07_deployment_security_assessment.md` §7; report §17.1) |
| Rev. 4 status as last primary-checked | **Initial public draft** — published 21 September 2026, comment period open through **30 November 2026** (`ledger/phase-10-v1-release-hardening/RESEARCH.md` §1, verified against the NIST CSRC page 2026-11-07) |
| This consolidation's check date | 2026-11-14 — **before the comment period closes**, so a "final Rev. 4" answer is not obtainable on this date by the ledger's own rule |
| Lookup performed in this loop | **NONE.** No network lookup was performed (the loop discipline; the P8.36 precedent — an unqueried page is an UNKNOWN, never recorded as a clear). No Rev. 4 delta is asserted from this record |
| Recorded disposition | Rev. 3 remains the baseline; Rev. 4 is DRAFT INPUT ONLY. Per the ledger step-3 rule, a go decision taken before 30 November 2026 **must carry the dated CSRC re-check as a P10.27 condition** — never assume the final matches the draft |
| Dated re-check procedure | On the review/go date: fetch https://csrc.nist.gov/pubs/sp/800/82/r4/ipd (and the CSRC publication index); if a final Rev. 4 has published, record its deltas against Rev. 3 and assess whether any delta changes a control in this release; if not, record that Rev. 3 remains the baseline with the draft status and the new check date | 

Acceptance criterion (c) is recorded honestly: the Rev. 4 status is
recorded as **draft-as-of-the-last-primary-check (2026-11-07), not
re-queried in this loop, with the re-check bound to the human review date
as a P10.27 condition** — this record does not claim a same-day CSRC check
it did not perform.

## 6. D6 scope coverage (ledger step 4) — D6 OPEN, UNSIGNED; every scope addition is a go blocker

`ledger/phase-09-pilot-qualification/06_ot_it_boundary_d6.md` records D6 as
**OPEN**: the review was not convened, no attendees/sign-off exist, and the
deployment the closure would cover is `<EMPTY>` (LR-031). The P10.16 step-4
rule — "if the release adds a site, network path, or adapter beyond the D6
scope, record that as a gap that blocks the go decision for that scope" —
applies here in its strongest form:

- There is **no signed D6 scope** to compare against. ANY site, network
  path, or adapter the release ships to is therefore beyond D6 scope, and
  the gap **blocks the go decision for that scope** until a signed,
  per-deployment D6 closure covers it (the P9.06 §6 closure PROPOSAL text
  is the prepared input; the decision is human).
- The scope-coverage check against the release scope defined in P10.01 §2
  (the five §25 capabilities; the §5 adapter matrix with every VERIFIED
  cell `<UNVERIFIED>`) is recorded as: **no coverage — D6 has no signed
  scope**. Acceptance criterion (d) is NOT met by this record; the gap is
  recorded as a go blocker, which is exactly what the criterion requires
  of an uncovered scope.
- The OpenEMS REST hop cleartext condition (P9.06 §3.A F5; cited as
  LR-029/LR-033/LR-088 in the P9.27 register) must be either closed or
  explicitly accepted in the signed D6 record — carried, not re-decided.

## 7. The consolidated deployment-facing cyber-risk table (the deliverable — trace-only)

Scope rule: every still-open SECURITY item from the four phase registers
plus the Phase 9 security design set is traced below. Items excluded from
this table are NOT dropped: Phase 7 O-17 (perf measurements) and O-18
(WASI 0.3 migration) are performance/platform-planning items carried in
their source register (and touch P10.17/P10.25 only insofar as any release
note must carry them); Phase 8 O-1/O-3/O-4/O-14/O-15 are research-track
items — relevant to this RELEASE only in the P9.27 §4 sense that no
research output is pilot-qualified and v1.0.0 excludes advanced
coordination (P10.01 §2.2); they remain OPEN in their source. The source
rows own the wording, the status, and the closure conditions; the "what
closes it" column is a pointer to the source's own recorded condition,
never a new promise. Owner cells carry the source register's owner/venue
verbatim where it named one; where the source left it empty, the cell
stays `<per source — EMPTY>`.

### 7.1 Phase 6 register — `ledger/phase-06-edge-protocol/30_retrospective_release_gate.md` §2

| Item | Source (path + id) | Current status (as recorded in source) | What closes it (per source) | Owner (per source) |
|---|---|---|---|---|
| D5 protocol/asset mix/pilot partner | phase-06 `30_retrospective_release_gate.md` §2 O-1 | OPEN — OpenEMS-first RECOMMENDATION | Pilot partner named; product + architecture review | Product + architecture review; `.agent/DECISIONS.md` |
| D6 cyber/OT boundary | phase-06 `30_retrospective_release_gate.md` §2 O-2 | OPEN — site review PREPARED, sign-off NOT SIGNED | Signed per-deployment closure (see §6 here) | Site security review board |
| D4 licenses (rustls RE-VERIFY; OpenEMS UNVERIFIED) | phase-06 `30_retrospective_release_gate.md` §2 O-3 | OPEN — UNVERIFIED until first scan | First human `cargo deny check` run | First human cargo-deny run |
| Uplink wire format (`BatchPayloadEncoder` REFUSES) | phase-06 `30_retrospective_release_gate.md` §2 O-4 | OPEN — telemetry does not yet leave the agent | Wire-format owner's implementation | Phase 7 / wire-format owner |
| TLS on the OpenEMS REST hop | phase-06 `30_retrospective_release_gate.md` §2 O-5 | OPEN — `https://` base URLs refused fail-closed | rustls REST transport written against a verifiable API; also a signed-D6 condition (P9.06 F5) | Integration engineer, at the lab run |
| Runner wiring of clock monitor + P6.14 composition | phase-06 `30_retrospective_release_gate.md` §2 O-6 | RECORDED for the composition owner | First-build wiring per the decision records | Coordinator follow-up at first build |
| Console panel embedding (`SitesEdgeStatusSection`) | phase-06 `30_retrospective_release_gate.md` §2 O-7 | RECORDED — component delivered, embedding point documented | Embedding at the composition point | Console owner |
| Authz action ids (`edge.status.read`, `edge.decommission`) | phase-06 `30_retrospective_release_gate.md` §2 O-8 | Interim recorded-drift gates with single documented swap points | The named swap at the reviewed change | Authz owner |
| Deferred protocols (IEEE 2030.5/CSIP, IEC 61850) | phase-06 `30_retrospective_release_gate.md` §2 O-9 | Deferred — NOT delivered this phase | Later adapters per §2.3 | Phase planning |
| Unbalanced three-phase modelling | phase-06 `30_retrospective_release_gate.md` §2 O-10 | Deferred per the Phase 4 record | Pilot need | Phase planning (pilot need) |
| Decommission storage wiring | phase-06 `30_retrospective_release_gate.md` §2 O-11 | None required this pass — revisit if the trait grows | Trait growth review | Storage owner |
| Registry DEFAULT-table flip follow-ups (O-12) | phase-06 `30_retrospective_release_gate.md` §2 O-12 | **CLOSED in the source** — listed here only to record the exclusion, not as an open item | — (closed at source) | — (closed at source) |

### 7.2 Phase 7 register — `ledger/phase-07-plugin-sdk/30_retrospective_release_gate.md` §2

| Item | Source (path + id) | Current status (as recorded in source) | What closes it (per source) | Owner (per source) |
|---|---|---|---|---|
| Workspace build + full test-suite run (the single verification gate) | phase-07 `30_retrospective_release_gate.md` §2 O-1 | UNRUN | First human `cargo test --workspace` | Human build/test pass |
| D1–D6 decisions OPEN | phase-07 `30_retrospective_release_gate.md` §2 O-2 | OPEN — SDK deferrals bounded by them | Product + architecture review | `.agent/DECISIONS.md` |
| License verifications (wasmtime + SDK dependency table UNVERIFIED) | phase-07 `30_retrospective_release_gate.md` §2 O-3 | UNPINNED-REVERIFY / ASSUMPTION | First lockfile-backed `cargo deny check` | Legal + engineering |
| `wasm-tools` 1.239.0 ASSUMPTION; wit-bindgen/cargo-component/wasip2 UNPINNED | phase-07 `30_retrospective_release_gate.md` §2 O-4 | ASSUMPTION (single secondary survey) | Release-page re-check at implementation | P7.27 CI owner / first-bindings human run |
| Rust-pin conflict note (wasmtime vs workspace Rust minimum) | phase-07 `30_retrospective_release_gate.md` §2 O-5 | RECORDED, unresolved-by-design | First-build `cargo check` outcome recorded | P7.10 owner at first build |
| Bindings generation (`generated-bindings` OFF; hostile-component run list open) | phase-07 `30_retrospective_release_gate.md` §2 O-6 | RECORDED seam | Gated-build human-gate list executed | Bindings-build owner + P7.13 verifier |
| Buf registration of `worker_api.proto` + first `buf lint`/`buf breaking` verdicts | phase-07 `30_retrospective_release_gate.md` §2 O-7 | RECORDED seam (job is config-only) | Activation wave runs the verdicts | Coordinator / activation wave |
| grpcio/protobuf pins un-installable; Python worker CI gates not wired by decision | phase-07 `30_retrospective_release_gate.md` §2 O-8 | OPEN (fail-closed CI state — see `18_ci_gate_run_record.md` §2) | Version verification + recorded pins | Activation wave |
| P7.22 seam-gap exposure (two path deps + integration test to apply at the witnessed run) | phase-07 `30_retrospective_release_gate.md` §2 O-9 | NOT APPLIED | Applied at the witnessed run; `check_dependency_rule.py` re-run | Coordinator/human at the P7.28 witnessed run |
| P7.28 exit-gate run + witness reproduction | phase-07 `30_retrospective_release_gate.md` §2 O-10 | NOT HELD — run book PREPARED/UNRUN | The witnessed run | Architect + named witness |
| P7.29 security clearance (findings tables EMPTY; R7-C2 wasmtime GHSA re-check open) | phase-07 `30_retrospective_release_gate.md` §2 O-11 | UNRUN | Human run + independent second-engineer re-run + the three GHSA advisory-page re-checks | Security engineer |
| `ILLEGAL_TRANSITION` audit nuance (wording vs behaviour) | phase-07 `30_retrospective_release_gate.md` §2 O-12 | FLAGGED | Extend the audit or correct the record | P7.07 owner at the human build pass |
| Per-instance grant binding (broker grants keyed by plugin id) | phase-07 `30_retrospective_release_gate.md` §2 O-13 | RECORDED recommendation | Verdict-scoped check hardening (optional) | P7.34/P7.38 owners |
| `builtin-ess-provider` manifest api-version inconsistency | phase-07 `30_retrospective_release_gate.md` §2 O-14 | RECORDED, not patched | Manifest fix at the reviewed change | P7.05 owner |
| Compromised-HOST posture; host↔control-plane transports UNAUTHENTICATED | phase-07 `30_retrospective_release_gate.md` §2 O-15 | RECORDED; real-TLS/PKI parts and behavioural proof remain open | P7.34 verifier: real mTLS stack, revoked-identity run | P7.34 verifier |
| Carried Phase 6 O-4/O-5 (wire format; OpenEMS REST TLS) | phase-07 `30_retrospective_release_gate.md` §2 O-16 | OPEN | See §7.1 rows O-4/O-5 | Phase 6 owners / wire-format owner |

### 7.3 Phase 8 register — `ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md` §4, plus the P8.25 and P8.36 records

| Item | Source (path + id) | Current status (as recorded in source) | What closes it (per source) | Owner (per source) |
|---|---|---|---|---|
| Security F-1 (HIGH): unguarded `torch.load(...)` at the GNN artifact load | phase-08 `30_retrospective_exit_gate.md` §4 O-5; full detail `ledger/phase-08-advanced-coordination/25_security_review.md` §2 F-1 | OPEN — blocks the with-framework activation (unreachable today: guarded import, no framework, no artifact) | Allowed format pinned in writing BEFORE the framework pin; hostile-artifact re-test at the P8.12 gate | P8.10/P8.11 owner; re-tested by the P8.12 human gate |
| F-2 (MEDIUM gap): MARL policy load site unwritten — safe-format precondition open | `ledger/phase-08-advanced-coordination/25_security_review.md` §2 F-2 | OPEN review-gap | Safe-load decision precedes the with-framework implementation | P8.13/P8.14 owner + human gate |
| F-3 (MEDIUM gap): ANDES case-format list unnamed; hostile-case behaviour UNVERIFIED | `ledger/phase-08-advanced-coordination/25_security_review.md` §2 F-3 | OPEN review-gap | Hostile-case runs + allowed-format list at the P8.16 gate | P8.16 owner |
| F-4 (INFO seam): artifact signing seam NOT implemented; manifests in-request | `ledger/phase-08-advanced-coordination/25_security_review.md` §2 F-4 | Seam referenced, not implemented | P8.31/P8.32 signing + P8.20 artifact store | P8.31/P8.32 wave |
| G-1/G-2 review gaps (no artifact-size cap; record-level training-data quarantine not built) | `ledger/phase-08-advanced-coordination/25_security_review.md` §2/§3 | OPEN gaps | P8.20 store activation; P8.35 deliverable | P8.20/P8.10/P8.13; P8.35 owner |
| Contract gaps S-1..S-3 (research workers not on the gRPC/mTLS transport; result-key sets narrow; no proposal dedup) | `ledger/phase-08-advanced-coordination/25_security_review.md` §6 | Recorded, never fixed in that record | Phase 7 contract owners' follow-ups | P8.04/Phase 7 contract owner |
| Advisory lookups NOT EXECUTED (every pin cell UNRUN; zero pins cleared) | `ledger/phase-08-advanced-coordination/36_cve_pin_check.md` §4/§6; register pointer phase-08 `30_retrospective_exit_gate.md` §4 O-7 | NOT-EXECUTED — an unqueried database is an UNKNOWN | The §3 procedure at pin time; re-check on any version move | Human gate at pin time |
| §19.4 numeric thresholds not pre-registered (X-8) | phase-08 `30_retrospective_exit_gate.md` §4 O-2 | OPEN — must be registered BEFORE the first run | Dated amendment pre-registration | Research lead |
| P8.24 drill RUN UNRUN (witness re-run of two cells) | phase-08 `30_retrospective_exit_gate.md` §4 O-6 | UNRUN | Independent witness run | Human gate |
| P8.28 CI jobs written, never run; Python gates fail-closed on the pin gaps | phase-08 `30_retrospective_exit_gate.md` §4 O-8 | UNRUN / fail-closed | Recorded pins land; the gates then run | Human gate |
| `research.status.read` authz action does not exist (interim deny-by-default gate) | phase-08 `30_retrospective_exit_gate.md` §4 O-9 | OPEN | The named swap at the reviewed change | flexcore-authz owner |
| No `research_controller` role in the P5.16 HELICS orchestrator | phase-08 `30_retrospective_exit_gate.md` §4 O-10 | OPEN | Additive role variant | sim-runner owner |
| Proto extensions needed (validity windows, tenant scope, research payloads) | phase-08 `30_retrospective_exit_gate.md` §4 O-11 | OPEN | Buf-governance extension | Phase 7 contract owner |
| torch/PyG and RL-framework DECISION-POINTs OPEN | phase-08 `30_retrospective_exit_gate.md` §4 O-12 | OPEN — dated decision not made | Product decision (never agent-closed) | Product |
| ANDES 2.0.0 GPLv3 legal confirmation gate G-4 OPEN | phase-08 `30_retrospective_exit_gate.md` §4 O-13 | OPEN — six-point isolation coded, legal sign-off pending | Legal sign-off | Human/legal gate |

Research-track items excluded from this cyber table by the §7 scope rule
(O-1 frozen results artifact, O-3 GNN scope extension, O-4 rule-based
baseline id, O-14 training-data inventory, O-15 PLAN location drift) stay
OPEN in `ledger/phase-08-advanced-coordination/30_retrospective_exit_gate.md`
§4 — they are cited there and in the P9.27 §4 carried-pointer table, and
none is closed by this consolidation.

### 7.4 The Phase 9 security design set (records 06/07/08/09/35/36) and the P9.27 register handles

The P9.27 register (`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md`)
is the stable LR-id handle for these records' open risks; the source
records own the wording. Cited by path and §:

| Record (path) | Register handles | Open security items traced (pointer) | Current status |
|---|---|---|---|
| `ledger/phase-09-pilot-qualification/06_ot_it_boundary_d6.md` §10 | LR-031–LR-038 | D6 OPEN; cleartext OpenEMS REST hop; signed-update DESIGN ONLY; secure boot undetermined; journal encryption unimplemented; H2/H3 UNRUN; Rev. 4 draft pressure | All OPEN; D6 UNSIGNED |
| `ledger/phase-09-pilot-qualification/07_deployment_security_assessment.md` §15 | LR-039–LR-046 | NO security assessment exists; production crypto absent (keyed checksum; FNV-1a-64); encryption at rest unimplemented; plugin-compromise/SBOM controls UNTESTED by construction; broker-duplication testing unnamed; secure boot/update verification unverified; Rev. 4 draft pressure; design positions must be consciously accepted | All OPEN; every assessment cell `<EMPTY>` |
| `ledger/phase-09-pilot-qualification/08_identity_access_approval.md` §12 | LR-047–LR-053 | No IdP integration; no step-up/MFA mechanism; approver role breadth; qualification-before-approval procedural; break-glass expiry procedural; no user/IdP/negative test; alarm-acknowledgement gap G3 | All OPEN |
| `ledger/phase-09-pilot-qualification/09_secrets_certs_keys.md` §12 | LR-054–LR-061 | No CA/secrets manager/HSM/certificates; production crypto absent; `secret-store:` reference gap; cert-expiry monitoring unimplemented; revocation propagation unmeasured; re-enrollment policy undecided; hardware key custody undetermined; vendor default credentials must be affirmatively excluded | All OPEN |
| `ledger/phase-09-pilot-qualification/35_safety_invariant_map.md` §8 | LR-152–LR-156 | Invariant-9 composition map-gap; ledger-step-2 pilot tests unscheduled; invariant-10 evidence is drill-only; checksum-grade primitives posture; head-gap sequence admission position | All OPEN |
| `ledger/phase-09-pilot-qualification/36_forged_command_drill.md` §9 | LR-157–LR-162 | Drill UNSCHEDULABLE (P9.23 tabletop UNHELD); alert-path leg depends on P9.25-verified infrastructure; five-case vocabulary only; checksum-grade primitives force a re-run; site-specificity; invariant-9 gap open under the drill's controls | All OPEN; drill UNRUN |
| `ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` (whole register) | LR-001…LR-211 + 42 carried pointers | The complete consolidation handle for every phase-9 open risk, cyber and operational; P10.26 must carry every limitation item-for-item | Version `pilot-limitations-register-v0.1-draft`; every acceptance cell `<EMPTY — UNSIGNED>` |

### 7.5 Phase 10 cyber-relevant constraints already recorded (cited, not re-decided)

| Item | Source (path) | Status |
|---|---|---|
| Edge journal encryption at rest is an OPEN item; journal paths in scope for OS-level site protection | `ledger/phase-10-v1-release-hardening/07_edge_offline_tests.md` §6 F-P10.07-3 | Recorded constraint — open |
| Edge command signature is the keyed-checksum scheme (tamper-EVIDENCE, not a guarantee); replacement is the recorded deployment requirement | `ledger/phase-10-v1-release-hardening/07_edge_offline_tests.md` §6 F-P10.07-4 | Recorded constraint — open |
| Candidate identifier `<PENDING P10.18>` in the P10.01 §6.1 freeze; any candidate change re-opens P10.01 and re-runs affected verifications | `ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md` §6 | PENDING — no candidate exists |

**Finding (count discrepancy, recorded honestly):** the P9.27 register file
on disk numbers its traced entries **LR-001…LR-211** (211 traced + 42
carried pointers; version `pilot-limitations-register-v0.1-draft`). The
P10.01 §10 row cites "LR-001..LR-225", which does not match the file. This
consolidation cites the register AS IT IS (LR-001…LR-211); reconciling the
count is the register owner's DATED AMENDMENT (the register's own §7
versioning rule), not an edit made here. No LR-id is renumbered, closed,
or re-described by this record.

## 8. The consolidated cyber risk statement for the release (ledger step 5) — TEMPLATE, no statement claimable

The ledger step-5 deliverable ("threats, controls, residual risks, and
their owners") requires assessment cells this consolidation cannot fill:

| Statement component | Source of record | State |
|---|---|---|
| Threats | `07_deployment_security_assessment.md` §9 (T1–T10, controls cited) | Design controls cited; evidence/status/residual `<EMPTY>` |
| Controls | `07_deployment_security_assessment.md` §10 (C1–C16, design positions cited) | Deployed status `<EMPTY>` |
| Residual risks | The P9.07 §12 findings register (EMPTY) + the §7 table here | No residual set is recordable — no assessment, no acceptance signatures |
| Owners | The source registers' owner cells (carried verbatim in §7) | Named persons: `<EMPTY>` — the deployment does not exist |

The only claimable consolidated statement is the negative one: **no
deployment-specific cyber risk statement exists, because no deployment,
no candidate build, and no assessment verdict exist. The go/no-go record
(P10.27) must treat §7's open items as OPEN and §6's D6 gap as a go
blocker for any scope a signed D6 does not cover.**

## 9. Findings recorded by this consolidation

| # | Finding | Disposition |
|---|---|---|
| C-1 | The §17.4 candidate re-check cannot run — no candidate build exists (P10.18 UNRUN); the drift procedure (§3) is prepared with EMPTY cells | Carried; runs against the P10.18 identity once it exists |
| C-2 | "No must-fix finding open" is unevaluable, not satisfied — the P9.07 findings register is EMPTY because no assessment exists (LR-039) | Carried to P10.25/P10.27 as an OPEN acceptance input |
| C-3 | D6 has no signed scope; every release scope addition is a go blocker for that scope (§6) | Carried to P10.27 as a recorded blocker |
| C-4 | Rev. 4 re-check bound to the human review date as a P10.27 condition; Rev. 3 remains the baseline; no lookup performed in this loop (§5) | Carried |
| C-5 | LR count discrepancy: the register file says LR-001…LR-211; P10.01 §10 cites LR-225 (§7 finding) | Register owner's dated amendment; not fixed here |
| C-6 | Nothing closed: zero items in §7 change status by virtue of this record | Binding on this record |

## 10. Reviewer-verification procedure (human)

1. Confirm §1's plain statement: this record closes nothing, assesses
   nothing, and every deployment-specific cell (§2) and drift/verdict cell
   (§3) is `<EMPTY>`/`<PENDING P10.18>`.
2. Spot-check five §7 rows against their source registers: the source path
   must be correct and the source row must still say what the Status cell
   reports. Any mismatch is a consolidation defect to fix here — never an
   edit to the source register.
3. Confirm §4: no must-fix/accepted finding is claimed closed; no
   acceptance signature appears anywhere in this record.
4. Confirm §5 against `RESEARCH.md` §1: Rev. 3 baseline, Rev. 4 draft with
   the 30 November 2026 comment close, no Rev. 4 delta asserted, the
   P10.27 re-check condition recorded.
5. Confirm §6: D6 OPEN/UNSIGNED per the P9.06 record; the every-scope-
   addition blocker is stated, not softened.
6. Confirm the C-5 finding: `grep -n "LR-225" ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md` and `grep -c "^| LR-" ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md` — the mismatch is recorded, not reconciled here.
7. Confirm no certification/safety/protection-grade wording appears, and
   the ledger tick reads "implemented, awaiting human build/test".

## 11. Filling commands (human, at execution time)

```sh
# After P10.18 records the candidate identity (fills RC-1 / P10.01 §6.1):
#   the identity values come from the P10.18 record — never assumed.

# The dated NIST CSRC re-check (on the actual review/go date — §5):
#   fetch https://csrc.nist.gov/pubs/sp/800/82/r4/ipd
#   and the CSRC publication index; record the deltas-or-no-final verdict
#   with the check date in §5 and carry/retire the P10.27 condition.

# D6 scope coverage (fills §6 for a real deployment):
#   convene the P9.06 review; sign the closure for the named deployment;
#   then re-check the release scope (P10.01 §2/§5) against the signed scope.

# Threat-control drift re-check (fills §3 RC-2..RC-4):
#   walk 07_deployment_security_assessment.md §9 against the CANDIDATE
#   build and configuration; record drift findings with owners.

# Registry verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 12. Status

**WRITTEN-UNRUN.** A consolidation record cannot be "run": the candidate
re-check, the deployment inspection, the D6 convening and signature, the
CSRC re-check, and the findings dispositions are the human gates and have
not happened. No candidate build exists; no deployment exists; nothing is
closed; nothing is accepted. The P10.16 ledger tick reads "implemented,
awaiting human build/test".
