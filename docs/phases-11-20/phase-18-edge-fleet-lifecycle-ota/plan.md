# Phase 18 — Edge Fleet Lifecycle and OTA Updates — Plan

Date: 2026-11-14. Status: **NOT STARTED** (0/16). Planning baseline only.
Ledger: `ledger_phase_18.md` (all boxes unticked). Conventions:
`../01_PROGRAM_CONVENTIONS.md` (binding). Context: `context.md` (seams and
constraints).

## 1. Scope statement

Phase 18 closes the largest recorded Phase 0–10 gap at DESIGN level: the
missing update-bundle producer (exit-gate record §5) becomes a specified OTA
pipeline — a TUF-style metadata repository whose device artifacts are
SUIT-shaped signed manifests [S-161][S-162] — plus the fleet-lifecycle
surface around it: the device update flow (download → verify → install →
status with explicit failure states; A/B slots; health-check auto-rollback;
anti-rollback counters), a staged cohort-rollout orchestrator with abort
thresholds and per-device attestation, update evidence into the audit chain,
device-identity lifecycle formalization (IDevID/LDevID mapped onto the P9.33
ceremony [S-165][S-166]), zero-touch onboarding design (FDO-pattern late
binding; the manual path stays), a decommissioning runbook, eSIM state as a
data model [S-167], fleet registry hardening on migration 0018, attestation
design with an honest hardware-absent fallback, a written-unrun update test
suite, and the update/command-lifecycle interlock. Compliance mapping to IEC
62443-4-1 SAR 3.4 / 4-2 CR 3.4 + CR 2.1 is recorded per deliverable
[S-163][S-164].

Three deliverable depths exist, stated up front so no record overclaims.
**Design-and-spec depth** (most subtasks): formats, flows, module designs,
runbooks, and written-unrun test suites — the WRITTEN-UNRUN class of the
Phase 0–10 base. **Record-only depth** (P18.01, P18.16): the ADR and the
decision/close records, because the honest deliverable there is the argued
decision. **Test-double depth** (P18.14): complete test files plus a mock
device, written and never run.

Two scope distinctions are load-bearing for this phase and restated in every
record that needs them: (1) the kernel PLUGIN staged rollout stays
cohort/percentage-free BY CONSTRUCTION (`crates/flexcore-kernel/src/revocation.rs`,
P7.38) — the device-fleet cohort orchestrator (P18.06) is a NEW,
separately-argued mechanism, not a reinterpretation of the kernel policy;
(2) the fail-closed P9.34 update seam is preserved by construction — a real
verifier is configured BEHIND the port; with nothing configured the agent
refuses updates exactly as shipped.

The expected artifact inventory, so the acceptance criteria in §8 are
checkable: an architecture ADR and E-decision register (P18.01, P18.16); a
bundle-format specification (P18.02); a repository role/key plan (P18.03); a
device-flow specification (P18.04); module designs for the edge extension,
orchestrator, and evidence pack (P18.05–P18.07); identity, onboarding,
decommissioning, and eSIM records (P18.08–P18.11); a registry migration
spec (P18.12); an attestation policy (P18.13); test files and a mock device
(P18.14); the interlock record (P18.15). Implementation-time `records/`
files are created per batch under the phase directory, never during
planning (conventions §8).

## 2. Subtask outline

| ID | Title | Summary | Key seams / sources |
|---|---|---|---|
| P18.01 | OTA architecture decision record | TUF-style metadata repository + SUIT-shaped signed manifests architecture; P18-E-1 registered: TUF-full vs minimal-signed-manifest first — DEFAULT: minimal signed manifest with TUF-shaped role separation designed-in, full TUF repository when the fleet exceeds a recorded threshold | [S-161][S-162]; `context.md` §1; conventions §4 |
| P18.02 | Update-bundle format specification | Versioned, signed format extending the P9.34 `UpdateManifest`: applicability metadata (vendor / device class / device IDs), sequence-number anti-rollback beyond the v1 minimum-version floor, payload digests, size caps | [S-162]; `crates/edge-agent/src/update_verification.rs` |
| P18.03 | Metadata repository implementation plan | Root offline key; targets with per-device-class delegations; snapshot; timestamp; N+1 root rotation [S-161]; expiry discipline; every key ceremony is a human gate (P10.21 interplay) | [S-161]; `ledger/phase-10-v1-release-hardening/21_signed_artifacts.md` |
| P18.04 | Device update flow specification | download → verify → install → status notifications incl. explicit failure states (OCPP 2.0.1 L01 firmware-update flow as a checklist — the failure-state names are enumerated at implementation from the OCA spec text, gap analysis §7 [S-121][S-122]); A/B slots per `UPDATE_SLOT_DECISION`; health-check via watchdog/boot counters; auto-rollback; anti-rollback counters | [S-161][S-162][S-121][S-122]; `crates/edge-agent/src/update_verification.rs` |
| P18.05 | Edge-agent update module extension | Real verifier behind the P9.34 seam — fail-closed preserved when unconfigured (`NoUpdateVerifier` semantics stay); manifest validation, sequence checks, update-progress state machine | `crates/edge-agent/src/update_verification.rs`; `context.md` constraint 1 |
| P18.06 | Staged rollout orchestrator (control plane) | Cohorts (canary 5–10% of a block per practice — a DEFAULT recommendation, not a sourced figure, pre-registered before any run), abort thresholds on failure rates, per-device attestation before authorization, fleet-state visualization data; explicit scope-distinction record vs the kernel plugin-rollout policy | `crates/flexcore-kernel/src/revocation.rs`; `context.md` constraint 2; conventions §4 |
| P18.07 | Update evidence + audit pack | Per-device update records into `flexcore-audit` (attempt / verify / install / result / rollback), digest-chained, exportable | `crates/flexcore-audit/`; P11.15 chain upgrade; LR-203 |
| P18.08 | Device identity lifecycle formalization | IDevID/LDevID model mapped onto the P9.33 ceremony; credential rotation schedule; revocation propagation (kernel revocation interplay) | [S-165][S-166]; `crates/edge-agent/src/enrollment_ceremony.rs`; `crates/flexcore-kernel/src/revocation.rs` |
| P18.09 | Zero-touch onboarding design | FDO-pattern late binding: ownership-voucher analog, rendezvous service seam, claim flow for factory-provisioned devices; the manual provisioning path (P9.33) stays the default and the fallback | [S-165][S-166]; `crates/edge-agent/src/enrollment_ceremony.rs` |
| P18.10 | Decommissioning runbook | Revoke operational certificates; release the ownership voucher; deregister from the fleet registry and protocol trust chains (P14 interplay, P14.03); data-wipe verification; second-life note | `crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql`; P14.03 |
| P18.11 | eSIM/cellular lifecycle tracking | Data model only: profile state, carrier-agnostic fields; carrier specifics out of scope | [S-167]; P18.12 schema |
| P18.12 | Fleet device-registry hardening | 0018 registry extension: firmware state, cohort membership, attestation state, last-seen health; migration spec additive, expand/contract per P11.18 | `crates/adapters/postgres/migrations/0018_edge_device_registry_p6_06.up.sql`; P11.18 |
| P18.13 | Attestation design | Boot-time evidence, measurement reporting, verification policy (what the platform checks before authorizing an update); hardware-absent fallback honestly specified (P18-E-2) | [S-166]; P18-E-2; P6.34 position (`ledger/phase-06-edge-protocol/34_secure_boot_position.md`) |
| P18.14 | Update test suite (written-unrun) | Mock device + manifest validation + rollback scenarios + anti-rollback negative tests + interrupted-update (power-loss) simulation + concurrent-command blocking tests | `crates/edge-agent/src/update_verification.rs`; conventions §1.3 |
| P18.15 | Update-safety interplay with command lifecycle | Updates BLOCKED while commands are ISSUED/TRACKING on the device; drain-before-update (report §5.6 discipline); post-update conformance re-check (edge conformance module interplay) | `crates/edge-agent/src/command_path.rs`; `crates/edge-agent/src/runner.rs`; `crates/edge-agent/src/conformance.rs` |
| P18.16 | Decision records + close | P18-E-1 (TUF-full vs minimal), P18-E-2 (hardware trust anchors assumption set), P18-E-3 (rollout cohort policy defaults); consistency audit; agents close none | conventions §4; `context.md` §3 |

## 3. Blocking order and shared surfaces

| Producer (blocks) | Consumers (what they take, never redefine) |
|---|---|
| P18.01 ADR + P18-E-1 default | P18.02 format, P18.03 repository plan, P18.14 tests |
| P18.02 format spec | P18.05 module extension, P18.06 orchestrator, P18.14 fixtures |
| P18.04 flow spec | P18.05 state machine, P18.14 rollback/power-loss scenarios, P18.15 |
| P18.08 identity formalization | P18.09 zero-touch claim flow, P18.10 decommissioning, P18.13 attestation subject |
| P18.12 registry schema (Agent B, wave 1) | P18.06 cohort membership, P18.11 eSIM fields, P18.10 deregistration |
| P18.13 attestation policy | P18.06 (per-device attestation before authorization), P18.14 negative tests |
| P18.15 blocking rule | P18.14 concurrent-command tests, P18.16 audit |
| P18.06 scope-distinction record | P18.16 (compiled into the close record) |

## 4. Deliberate non-goals (this phase does not)

- No CA is stood up and no PKI service is operated — issuance, revocation,
  and renewal remain P13/P14 seams and DEPLOYMENT REQUIREMENTS (P14.03
  interlock; gap analysis §4 PKI row).
- No device-firmware implementation: A/B mechanics, watchdogs, and boot
  counters are device concerns the platform SPECIFIES (interfaces, evidence
  contracts, thresholds) but cannot implement.
- No carrier-specific eSIM integration — profile state is data only [S-167].
- No fleet exists: no device is enrolled, claimed, updated, rolled out, or
  decommissioned; every flow runs against test doubles only.
- No change to the kernel plugin-rollout policy (stays cohort/percentage-
  free BY CONSTRUCTION) and no change to the fail-closed P9.34 default
  semantics.
- No dispatch-authority change: the never-granted set stays byte-identical;
  updates ride the existing authorization gates, never around them.

## 5. Wave plan

Two agents, per the standing loop discipline.

| Wave | Agent A | Agent B | Shared note |
|---|---|---|---|
| 1 | P18.01–P18.07 (OTA pipeline: ADR, format, repository plan, flow, edge module, orchestrator, evidence) | P18.08–P18.13 (identity/registry/attestation: lifecycle, zero-touch, decommissioning, eSIM, registry, attestation) | Disjoint seams: update path vs identity path; P18.12's schema is defined by Agent B and consumed by Agent A's P18.06 cohort membership; P18.06's scope-distinction record compiles toward P18.16 |
| 2 | P18.14 + P18.16 (test suite, decision records, close) | P18.15 (command-lifecycle interlock) plus review of the wave-1 honesty statements | P18.14's concurrent-command tests consume the P18.15 blocking rule; the consistency audit covers every phase-18 artifact |

Dependency rules inherited (gap analysis §9): Phase 18 is mostly independent
but consumes the P11 audit chain and the P14 security patterns — trace_id,
audit chain, permission sets are consumed, never redefined. Each implemented
batch gets a decision record under `records/` and keeps the dependency-rule
checker green (conventions §2, §6.1).

### 5.1 Batch discipline during implementation

- Every implemented batch closes with a decision record under `records/`
  citing the subtask IDs, the seams touched, and the honest status
  ("implemented, awaiting human build/test") — conventions §2.4.
- Thresholds named in designs (abort thresholds, cohort sizes, expiry
  windows) are registered BEFORE any run that could fill them, and dynamic
  results stay structurally separate (X-8 carryover; conventions §5).
- Phase close follows conventions §2.5: HANDOVER, CHECKLIST, and the
  close/exit-gate record are added under this directory only at
  implementation completion (P18.16) — never during planning.
- New crates named in designs (an update-repository service, a rollout
  orchestrator crate) become workspace members only in the batch that adds
  their real code, with the additive `tools/check_dependency_rule.py`
  update that requires (conventions §6.2).

## 6. Decision points (E-decisions — agents never close them)

| ID | Question | Options | DEFAULT / RECOMMENDATION | What would close it | Closer |
|---|---|---|---|---|---|
| P18-E-1 | TUF-full metadata repository from day one, or minimal signed manifests first? | (a) full TUF repository (all roles, rotation, expiry from the start); (b) minimal signed manifest with TUF-shaped role separation designed-in, full repository when the fleet exceeds a recorded threshold; (c) plain signed bundles without role separation (rejected — no key-compromise recovery path) | **DEFAULT: (b)** — minimal manifest first, role separation designed-in, full TUF repository gated on a recorded fleet-size threshold | The owner recording the threshold and the operational commitment to run repository expiry/rotation discipline | human owner |
| P18-E-2 | Hardware trust anchors (TPM / secure element): what is assumed, and what changes without them? | (a) hardware-optional design — attestation degrades to software evidence, the difference stated honestly per case; (b) hardware-required (rejected for 0.x — no fleet, no hardware spec); (c) ignore hardware (rejected — dishonest) | **DEFAULT: (a)** — the assumption set is recorded (PENDING PILOT HARDWARE SPEC per the P6.34 posture); every attestation statement says what the platform checks WITH and WITHOUT an anchor | The pilot hardware spec (P6.34 cells) filling the determination | human owner |
| P18-E-3 | Rollout cohort policy defaults: canary size, block progression, abort thresholds, pause-vs-abort semantics | (a) canary 5–10% → progressively larger blocks, failure-rate abort thresholds pre-registered; (b) operator-manual per-block approval only; (c) full-fleet simultaneous (rejected — no staged safety evidence) | **DEFAULT: (a)** — all values are configuration and pre-registered (X-8) before any run; the mechanism is separate from the kernel plugin-rollout policy (context constraint 2) | The owner recording the thresholds after reviewing the P18.06 design | human owner |

## 7. Risks

| # | Risk | Basis / mitigation |
|---|---|---|
| R-P18-1 | No physical devices exist to test against — every flow is validated against test doubles only | P18.14 uses a mock device; nothing in any record claims hardware behavior; every hardware determination stays PENDING PILOT HARDWARE SPEC (P6.34 posture) |
| R-P18-2 | Signing key ceremonies are human-gated — no production key exists, so the repository/manifest pipeline cannot be exercised for real | Key ceremonies are recorded as human gates (P10.21 interplay); test-grade signing stays labeled test-grade and never substitutes |
| R-P18-3 | Embedded update mechanics (A/B slots, watchdogs, boot counters) are device-firmware concerns the platform can specify but not implement | The flow spec (P18.04) defines the interface and evidence contract; device-side behavior is a deployment/hardware requirement, recorded not built |
| R-P18-4 | TUF operational maturity — running a metadata repository is an ongoing operational commitment (expiry discipline, rotation rehearsals, key ceremony logistics) | P18-E-1's DEFAULT starts minimal with role separation designed-in; the operational burden is a recorded closing condition, not an assumption |
| R-P18-5 | The 0–10 base is WRITTEN-UNRUN — the first build may surface recorded defects (e.g., F-P10.07-5), and the P9.34 seam itself is unverified | Program-level standing risk (program README boundary 1); planning proceeds, implementation cannot precede a green human build |
| R-P18-6 | Honesty wording — any text implying a fleet exists, an update was produced/applied, or a trust anchor is present is a compliance failure | Context constraint 6 + §8 acceptance criteria; every phase-18 record carries the not-produced / not-applied / no-fleet statement |

## 8. Acceptance criteria (honest)

1. Format specifications, flow specifications, orchestrator and module
   designs, runbooks, policy records, and test files exist **written-unrun**;
   no subtask claims a run, a test pass, or verified behavior.
2. **No update bundle is produced, signed, staged, or applied** — the
   P9.34/P10.12 gap is closed at DESIGN level only, and the close record
   says so verbatim.
3. **No device exists**: nothing is enrolled, claimed, updated, rolled out,
   attested, or decommissioned; all validation is test doubles.
4. **Fail-closed preserved by construction**: with no configured verifier
   the agent refuses updates (`NoUpdateVerifier` semantics unchanged); the
   close record quotes the refusal behavior as specified.
5. The kernel plugin-rollout policy stays **byte-identical**; the
   device-fleet cohort mechanism is recorded as a separate,
   separately-argued mechanism (context constraint 2) in P18.06 and P18.16.
6. Updates are specified as **refused while any command is ISSUED/TRACKING**
   on the device (P18.15); no update path bypasses the command lifecycle or
   the authorization gates.
7. **No production signing key exists**; test-grade signing stays labeled
   test-grade; every key ceremony is a human gate (P10.21 interplay).
8. `python tools/verify_docs.py` and `python tools/validate_agent.py` are
   green after every documentation change in this phase (results quoted,
   never asserted).
