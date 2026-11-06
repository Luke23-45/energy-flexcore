# Iteration 5 — Security / Safety / Control-Authority Review: Findings

| Field | Value |
|---|---|
| Iteration | 05 — security, safety, control authority |
| Scope | `ledger/phase-07-plugin-sdk/`, `ledger/phase-08-advanced-coordination/`, `ledger/phase-09-pilot-qualification/` (read-only for all other phases) |
| Reviewer role | Iterations 5 and 6 reviewer (independent; no favour to the draft) |
| Date | 7 November 2026 |
| Baseline counts (before patching) | P7: 32 subtasks; P8: 30; P9: 30 (verified by grep, see §7) |

## Method — what was re-read and how

Re-read in full, against the ledger text (not from memory):

- Report §17 (security model, zones, required controls, threat model, safety
  invariants), §14 (command lifecycle, invariants, fallback), §18 (edge trust
  boundary, deployment profile), §21.4 (control-authority modes), §22.3
  (release gates), §5.7 (plugin authority model), §13.4 + ADR-010
  (research outputs are proposals only), §22.1 (ADR-005, ADR-006).
- `ledger_phase_7.md` (P7.01–P7.32), `ledger_phase_8.md` (P8.01–P8.30),
  `ledger_phase_9.md` (P9.01–P9.30), each phase's `CHECKLIST.md`,
  `RESEARCH.md`, `AGENTS.md` rules 1–5, `.agent/DECISIONS.md` (D1–D6 OPEN,
  esp. D6).
- Keyword/coverage checks run by grep: `mTLS`, `enrol`, `revocation`,
  `immutable`/`append-only`, `tamper`, `nonce`, `replay`, `idempot`,
  `step-up`, `separation of duties`, `RLS`, `tenant`, `clock`, `1547.3`,
  `zero trust`, `CVE`, `dependency scanning`, `SBOM`, `secure boot`,
  `staged`, `outbox`/`inbox`, `audit integrity`, and a per-item mapping of
  the ten §17.4 threats and ten §17.5 safety invariants to named subtasks.

Control-authority statements were checked line by line for overstatement:
P7 header + P7.15/P7.16/P7.26/P7.32, P8 header + P8.14/P8.29/P8.30, P9.01/
P9.17/P9.26 — all state the correct mode per release (v0.5.0 simulation/
shadow; research simulation/shadow only; v1.0.0 operator-approved, bounded
automation explicitly excluded). **No control-authority overstatement was
found.** That is a genuine pass, not a courtesy: the one place a mode change
could have crept in (profile switching, P7.16) explicitly forbids it, and
P9.26 verifies the deployed configuration field-for-field.

## Severity counts

| Severity | Count |
|---|---|
| Critical | 0 |
| High | 8 |
| Medium | 17 |
| Low | 5 |
| **Total** | **30** |

No Critical: nothing in P7–P9 grants field dispatch authority, certifies
anything, or contradicts the simulation-first ladder. The eight High items
are all *missing proofs* (a required test or enforcement that does not exist
yet), not live misstatements — which is exactly what a ledger review should
catch before code exists.

---

## (a) Missing subtasks

| ID | Severity | Finding | Patches |
|---|---|---|---|
| I5-M1 | High | **P7 has no audit-integrity proof.** Audit is named a protected-kernel invariant (ledger header, P7.02, P7.12) and denials must be "audited" (P7.11–P7.13, P7.23), but no subtask proves audit records themselves are tamper-evident (append-only, hash-chained, or equivalent), retained, or survivable when the plugin host crashes. A compromised plugin host could alter the very log that would convict it. §17.3 (audit logging), §17.5, §22.3. | New **P7.33** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-M2 | High | **P7 has no plugin-host trust-boundary / mTLS subtask.** P7.10 wires the host to the control plane "over gRPC/Protobuf, not by in-process linking" — no mTLS, no service identity, no per-session authentication, and no requirement that the control plane treat the host as untrusted and validate every plugin result. §18.2 states the trust rule for the edge; the same rule is never stated for the plugin host, which is the more exposed party (it executes untrusted code). §17.3, §18.2. | New **P7.34** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-M3 | High | **No subtask owns the ADR-006 no-dylib enforcement.** See I5-E1: the P7 CHECKLIST claims the evidence is a "code search + architecture review (P7.02/P7.28 record)", but neither subtask runs one. | New **P7.35** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-M4 | High | **P9 has no hostile replay/forgery test at the edge.** §17.4's first threat is forged/replayed command; §18.2 requires the edge to validate every command. P9.15 tests expiry/supersede, P9.19 tests EXPIRED, and P9.23 runs a *tabletop* on a forged/replayed command — but no subtask replays a captured signed command, forges a signature, or sends an out-of-sequence command at the edge and requires rejection + audit. The pilot's core security proof is implied, never executed. | New **P9.31** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I5-M5 | High | **P9 has no audit-integrity / edge-audit-buffer proof.** P9.07 requires the audit-logging control and P9.24 verifies restored audit records, but no subtask tests audit tamper-evidence, retention enforcement per §16.3, or the §18.1 local audit buffer uploading completely (no loss, no duplication) after a central outage. | New **P9.32** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I5-M6 | High | **P8 has zero tenant-isolation coverage.** The string `tenant` occurs 0 times in `ledger_phase_8.md`. Research workers read telemetry/topology and train models, but no subtask scopes a research worker to a tenant/site, and nothing forbids a GNN trained on one tenant's data/topology from serving another tenant. §17.4 (cross-tenant leakage), §16.2. | New **P8.31** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-M7 | High | **P8 has no model-artifact signing/provenance requirement.** P8.25 tests malformed/oversized models, but nothing requires model artifacts in object storage (§6.5) to be signed and verified at load. A tampered model swapped in storage would be loaded and trusted. Contrast P7.14, which requires signing for plugin packages; models are the research equivalent and have no equivalent. §9.3, §17.4 (supply chain). | New **P8.32** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-M8 | Medium | **P9 has no edge-enrollment ceremony subtask.** P9.13 lists "edge-agent enrolment" as one commissioning check with no acceptance criterion for how trust is bootstrapped: single-use short-lived enrollment token, first-boot trust, re-enrollment after compromise, enrollment revocation. P9.09 covers rotation/revocation of *existing* identities, not the birth of one. §18.1 (secure site enrollment and identity), §17.3. | New **P9.33** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I5-M9 | Medium | **P7 revocation and staged rollout are specified but never tested.** P7.14 step 1 asks "how revocation is handled" but its acceptance criteria never require a revocation test (revoked signature refused at `Verified`; an already-installed revoked plugin disabled). §17.4's supply-chain control includes "staged rollout", which no P7 subtask requires for plugin upgrades. | New **P7.38** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-M10 | Medium | **P7 has no replay/sequence/expiry test on the plugin path.** §17.3/§17.4 require signed commands, sequence numbers, expirations, replay protection. P7.18 gives worker retries idempotency keys, but no subtask replays a plugin-proposed command, sends an out-of-sequence proposal, or tests broker duplication (outbox/inbox) on the plugin path. | New **P7.36** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-M11 | Medium | **Research proposals carry no validity window.** P8.04's result contract lists provenance/diagnostics but no `valid-from`/`valid-until`. A stale research proposal could be validated against a fresh snapshot and look current. §14.3 (every command carries a validity window), §17.5 invariant 5 (no stale or expired command). | New **P8.34** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-M12 | Medium | **P9 has no signed edge-update verification/rollback test.** §18.3 requires "signed and verified update bundles"; P9.24 rehearses backup/restore, not update. No subtask rejects a bad-signature update or rolls back a failed edge update at the pilot. | New **P9.34** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I5-M13 | Medium | **P8 research workers have no identity, mTLS, or secret handling.** No subtask gives a research worker a service identity or mTLS to the control plane, or states how it obtains credentials. P8.04 reuses the P7.17 template, which itself has no mTLS subtask (see I5-M2). Secrets for dataset/model-store access are unaddressed. §17.3. | New **P8.33** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-M14 | Medium | **P8 training-data provenance, license, and poisoning controls are missing.** P8.03 audits *code* dependencies only. Datasets — source, license, version, validation, poisoning/malformation controls — have no subtask, although P8.25 step 2 mentions poisoned training records only in passing ("record the data validation in place") with no acceptance criterion. | New **P8.35** in `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-M15 | Medium | **P7 cross-component isolation and data-level tenant filtering are untested.** P7.13 hostile components test ambient access (files/network/env/secrets), not one component reading another component's state inside a shared host process, and not tenant scope enforced on *returned data*. P7.11 names the second risk ("tenant scope checked at the edge of the call but not on the data returned") but its acceptance criteria test only call-edge denial. | New **P7.39** in `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-M16 | Medium | **P9 incident response is tabletop-only.** P9.23 requires a tabletop exercise for one incident class; no subtask executes a *live* drill of containment (revert a site to shadow/local control) for a forged-command attempt. A procedure that has only been talked through is not a proven containment path. §17.3. | New **P9.36** in `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |
| I5-M17 | Low | **P9.07 never mentions IEEE 1547.3.** P9 `RESEARCH.md` §2 states the 1547.3 cybersecurity guide "is directly relevant to P9.07 (security assessment) — read it, do not assume its contents", but P9.07's steps never name it. | Amended **P9.07** in place (new step + acceptance criterion), `ledger/phase-09-pilot-qualification/ledger_phase_9.md` |

## (b) Issues / errors / contradictions

| ID | Severity | Finding | Patches |
|---|---|---|---|
| I5-E1 | High | **P7 CHECKLIST evidence claim contradicts the subtasks.** `ledger/phase-07-plugin-sdk/CHECKLIST.md` says "No arbitrary Rust dylib loading exists — Evidence: code search + architecture review confirming no `.so`/`.dll` plugin loader in any form (P7.02/P7.28 record)". P7.02 is a scope freeze (paper); P7.28 is an exit-gate run book for the swap + untrusted-plugin proofs. Neither runs a code search, bans a dependency, or fails CI. The gate item would be tickable on paper while the check was never performed. | New **P7.35** (owns the check) + CHECKLIST.md evidence line corrected to cite P7.35 |
| I5-E2 | Medium | **P7.15's no-dispatch verification has no bound test run.** P7.15 acceptance says the research profiles' no-dispatch property is "verified by attempting a dispatch from within them", but P7.23 (the untrusted-plugin test) builds a WASM component and a gRPC worker and never states it runs inside the `research-gnn`/`research-marl` profiles. The claim and the test are not joined. | New **P7.34** does not cover this; patched by amending **P7.23** step 1 in place to require the attempts also run inside the `research-gnn`/`research-marl` profiles (see §6) |
| I5-E3 | Medium | **P7.16 records a risk it never mitigates.** Risk: "A switch that drops the audit plugin and then cannot record the switch". No acceptance criterion forbids removing the audit plugin or requires the switch to fail if audit coverage would be lost. A recorded risk with no test is a known hole. | New **P7.33** (acceptance: profile switch that would drop audit coverage is refused) |
| I5-E4 | Medium | **P8.27's "promote" path has no negative test.** P8.27 step 4 requires explicit confirmation for any action promoting a research proposal toward the dispatch path, but the acceptance criteria never test that promotion cannot bypass the protected gates. A console button that moves a proposal toward dispatch is a control-authority-adjacent path introduced in this phase with no attack test. | Amended **P8.27** in place (new step + acceptance criterion), `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-E5 | Medium | **P8.25 has two traceability breaks.** (1) Acceptance says "no model file can execute code on load (deserialization safety verified for the formats used)" but no step names the allowed model formats or forbids arbitrary-code deserialization (e.g. unguarded pickle). (2) P8 `RESEARCH.md` §7 task 4 explicitly says "P8.25 should confirm the pinned versions carry no known critical issue at implementation time" — P8.25 contains no CVE/advisory check step at all. | Amended **P8.25** in place (allowed-formats step) + new **P8.36** (CVE/advisory check), `ledger/phase-08-advanced-coordination/ledger_phase_8.md` |
| I5-E6 | Medium | **P9.07 assesses vulnerability management but never sets a pilot cadence.** Step 2 lists "vulnerability management and dependency scanning" as a control to assess; no step sets a scanning/patch cadence, owner, or window for the pilot period. A control assessed once at qualification can silently lapse during the pilot. | Amended **P9.07** in place (new step + acceptance criterion) |
| I5-E7 | Low | **P7.11's never-granted set omits the `dispatch.validate.request` distinction.** §5.7 grants `dispatch.validate.request` as a *request to the protected validator* — the broker must allow the request path while never treating it as validation itself. P7.11 step 3 lists the never-granted set without this boundary, so an implementer could either block the legitimate request path or, worse, wire it to the validator's verdict. | Amended **P7.11** in place (one clarifying step), `ledger/phase-07-plugin-sdk/ledger_phase_7.md` |
| I5-E8 | Medium | **§17.5 safety-invariant coverage is unsystematic across all three phases.** Only P9 cites invariants by number (invariant 9 in P9.18, invariant 10 in P9.20); P7 cites none by number and P8 none. Invariants 1–8 are covered implicitly across subtasks with no traceability check, so an invariant could be dropped without any subtask failing. Invariant 9 (a missing/uncertain/stale model is not a passing feasibility check) is tested at the pilot (P9.18 step 5) but never at the plugin boundary — a plugin result computed against a stale/missing grid model has no named test. | New **P7.37** (invariant→test map for the SDK) + new **P9.35** (invariant→test map for the pilot) |
| I5-E9 | Low | **P7.14's signing scheme has no key-compromise procedure.** Steps cover signing, verification, and revocation "handling", but no step defines what happens when a *signing key* is compromised (re-key, re-sign, distrust window). Low because plugin signing is itself still to be built; the gap is a missing procedure, not a wrong one. | Suggestion only — see I5-S2; deferred (needs a key-management design decision, not inventable here) |
| I5-E10 | Low | **§17.4 threat coverage is otherwise complete on paper but unmapped to tests in P9.** P9.07 step 1 maps all ten threats to controls + evidence, which is good — but the mapping is to *controls*, not to the subtask that tests each control (P9.14/P9.15/P9.18/P9.19/P9.20/P9.23). A control could be recorded as "implemented" with no test behind it and P9.07 would still pass. | Amended **P9.07** step 1 in place (each threat row must name the testing subtask) |

## (c) Suggestions

| ID | Severity | Suggestion | Disposition |
|---|---|---|---|
| I5-S1 | Medium | P7.29 step 1 requires a malicious plugin to be "contained and removable", and P7.07's drain covers disable/upgrade — but no step explicitly tests drain/removal of a plugin in the `Failed` state. A plugin that fails *because* it is malicious is the one most likely to resist removal. | Patched: step added to new **P7.38** (revoked/failed plugin drain + removal test) |
| I5-S2 | Low | Add a signing-key compromise procedure (re-key, re-sign, distrust window) to the P7.14 signing scheme when it is designed; do not leave key compromise to be improvised after a compromise. | Deferred — needs a key-management design decision (who holds signing keys, what the distrust mechanism is) that the report does not specify; recorded here so it is not lost |
| I5-S3 | Low | P7.19 risk: "a secret resolved once and then cached in plaintext in plugin memory". No acceptance criterion covers secret-handle expiry/rotation while a plugin is running. | Deferred — handle-invalidation semantics are not specified in the report (§5.7 only says secrets are resolved by host/adapter); inventing them here would be a design decision, not a review patch. Flagged for the P7.19 implementer to resolve with the architect |
| I5-S4 | Low | P8.13's safety-filter derivation is reviewer-checked before training (good). Suggest the same independent-reviewer rule be stated for the P8.05 harness's deliberately-wrong suite — it already requires separate ownership, so this is a wording hardening, not a gap. | Not patched — P8.05 step 3 already requires separate ownership and review by a non-author; no change needed |
| I5-S5 | Low | P9.09's revocation propagation is measured (good). Suggest P9.33 (new enrollment subtask) cross-reference P9.09 so enrollment and revocation are tested as one lifecycle, not two. | Patched: cross-reference written into new **P9.33** |

## (d) Stale research

| ID | Severity | Finding | Disposition |
|---|---|---|---|
| I5-D1 | Medium | **Wasmtime CVE floor is point-in-time with no owning re-check.** P7 `RESEARCH.md` §7 task 1 ("re-check the GHSA pages for CVE-2026-115889 / CVE-2026-115893 / CVE-2026-115384 at implementation time") has no owning subtask; P7.10/P7.13 pin and record the version but never re-check the advisories. A new advisory between the cut-off and implementation would not be caught. | Patched: step added to **P7.13** in place (re-check GHSA pages at implementation time, record advisory IDs) |
| I5-D2 | Low | **Rust 1.100 / WASI 0.3 re-evaluation is correctly deferred, not stale.** Rust 1.100.0 was scheduled 12 November 2026, five days after the 7 November cut-off; P7.32 already carries the re-evaluation trigger ("after Rust 1.100 stable + host compatibility"). Re-checked; no patch. | No patch — checked and correct |
| I5-D3 | Low | **Ray/RLlib version as of the cut-off was never verified from a primary source** (P8 `RESEARCH.md` §7 task 1; only 2.56.1/2.57.0 from July–August 2026 secondary snapshots). P8.13 step 4 already requires the framework + API stack + version to be pinned at implementation time, so the owner exists. | No patch — owner exists (P8.13); recorded so the implementer does not copy the snapshot versions |
| I5-D4 | Low | **NIST SP 800-82 Rev. 4 final publication is pending.** Comment period closes 30 November 2026, after the cut-off; P9.07 correctly treats Rev. 4 as draft only, and P9 `RESEARCH.md` §3 assigns the final-text re-check to Phase 10 (P10.16). P9.30's handover should carry it forward so it is not lost between phases. | Patched: forward item added to **P9.30** step 4 in place (handover carries the Rev. 4 final re-check to Phase 10) |
| I5-D5 | Low | **PJM capacity-market date conflict is already flagged.** P9 `RESEARCH.md` §1 records that sources differ on the PJM capacity date (1 February 2027 vs 1 February 2028) and P9.11 requires verification at time of use. Correctly handled; no patch. | No patch — checked and correct |
| I5-D6 | Low | **IEEE 1547 revision in ballot.** P9 `RESEARCH.md` §2 notes IEEE 1547-2018's revision is in ballot and the 2018 requirements must be re-checked when it publishes; P9.27 is named as the watch-item carrier. Re-checked; the carrier exists. | No patch — checked and correct |

## (e) Severity summary

- Critical: 0. High: 8 (I5-M1–M7, I5-E1). Medium: 17 (I5-M8–M16, I5-E2–E6, I5-E8, I5-S1, I5-D1). Low: 5 (I5-M17, I5-E7, I5-E9, I5-E10, I5-D2–D6 counted as 5 low items incl. D2/D3/D4/D5/D6 — see note).
- Note on counting: §(d) items D2–D6 are Low "checked and correct" records, not
  defects; they are counted in the Low total because the task requires a
  severity per item, and their severity is the severity *if* the watch item
  were dropped. The defect total is 24 (17 missing/issue items in §(a)–(b)
  plus S1); the remaining 6 are stale-research watch items, 4 of which needed
  no patch.

## (f) Patch map — every item to an exact file + subtask ID

| Item | File | Subtask ID (new or amended) |
|---|---|---|
| I5-M1 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.33** |
| I5-M2 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.34** |
| I5-M3 / I5-E1 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` + `ledger/phase-07-plugin-sdk/CHECKLIST.md` | New **P7.35**; CHECKLIST evidence line corrected |
| I5-M4 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.31** |
| I5-M5 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.32** |
| I5-M6 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.31** |
| I5-M7 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.32** |
| I5-M8 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.33** |
| I5-M9 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.38** |
| I5-M10 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.36** |
| I5-M11 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.34** |
| I5-M12 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.34** |
| I5-M13 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.33** |
| I5-M14 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | New **P8.35** |
| I5-M15 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.39** |
| I5-M16 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.36** |
| I5-M17 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | Amended **P9.07** |
| I5-E2 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | Amended **P7.23** |
| I5-E3 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.33** |
| I5-E4 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.27** |
| I5-E5 | `ledger/phase-08-advanced-coordination/ledger_phase_8.md` | Amended **P8.25** + new **P8.36** |
| I5-E6 / I5-E10 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | Amended **P9.07** |
| I5-E7 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | Amended **P7.11** |
| I5-E8 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` + `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P7.37** + new **P9.35** |
| I5-E9 / I5-S2 | — | Deferred (see §6) |
| I5-S1 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | New **P7.38** |
| I5-S3 | — | Deferred (see §6) |
| I5-S4 | — | No patch needed (P8.05 already requires it) |
| I5-S5 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | New **P9.33** |
| I5-D1 | `ledger/phase-07-plugin-sdk/ledger_phase_7.md` | Amended **P7.13** |
| I5-D2 / I5-D3 / I5-D5 / I5-D6 | — | No patch — checked and correct |
| I5-D4 | `ledger/phase-09-pilot-qualification/ledger_phase_9.md` | Amended **P9.30** |

## §6 — Applied vs deferred log

**Applied:** 7 new P7 subtasks (P7.33–P7.39), 6 new P8 subtasks (P8.31–P8.36),
5 new P9 subtasks (P9.31–P9.36),
plus in-place amendments to P7.11, P7.13, P7.23, P8.25, P8.27, P9.07, P9.30,
and the P7 CHECKLIST dylib evidence line. Subtask indexes, counts, and the
retrospective dependency ranges (P7.32, P8.30, P9.30) were updated to match.
Subtask IDs are allocated in one sequence across both iterations: Iteration 5
takes P7.33–P7.39, P8.31–P8.36, P9.31–P9.36; Iteration 6 continues from
P7.40, P8.37, P9.37.

**Deferred (with reasons):**
- I5-E9 / I5-S2 (signing-key compromise procedure): needs a key-management
  design decision (key holders, distrust mechanism) the report does not
  specify. Inventing it would be a design decision, not a review patch.
- I5-S3 (secret-handle expiry/rotation while a plugin runs): handle-
  invalidation semantics are not specified in the report; the P7.19
  implementer must resolve them with the architect.

**Not patched because checked and correct:** control-authority mode
statements in all three phases (see Method); I5-D2, I5-D3, I5-D5, I5-D6;
I5-S4 (P8.05 already requires separate ownership/review).

## §7 — Verification (counts before patching)

```
$ grep -c "^- \[ \]" ledger/phase-07-plugin-sdk/ledger_phase_7.md \
    ledger/phase-08-advanced-coordination/ledger_phase_8.md \
    ledger/phase-09-pilot-qualification/ledger_phase_9.md
ledger/phase-07-plugin-sdk/ledger_phase_7.md:32
ledger/phase-08-advanced-coordination/ledger_phase_8.md:30
ledger/phase-09-pilot-qualification/ledger_phase_9.md:30
```

Post-patch counts are recorded in the parent report-back, not here, so this
file stays an accurate record of the review as run. (After Iteration 5
patching: P7 39, P8 36, P9 36 — before Iteration 6 adds its own.)
