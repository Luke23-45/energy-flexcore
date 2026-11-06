# Iteration Log — FlexCore Implementation Ledger

Ledger cut-off: 7 November 2026. Each iteration has a `findings.md` in
its own directory with missing subtasks, issues/errors/contradictions,
suggestions, stale research, severity per item, the exact file +
subtask ID each item patches, and an applied-vs-deferred log.

Severity: **Critical** = would make a release decision unsafe;
**High** = a wrong claim a subtask would act on, or a missing proof for
a gate; **Medium** = stale detail a re-verify rule would catch;
**Low** = precision polish or an already-flagged open verification.

## Summary

| Iteration | Date | Angle | Found (C/H/M/L) | Patched | Deferred |
|---|---|---|---|---|---|
| 01 | 2026-11-07 | Build pass (initial construction) | 0 / 7 / 13 / 4 = 24 | 23 | 1 |
| 02 | 2026-11-07 | Deep re-analysis (re-read report, re-ran searches, robust patches) | 0 / 0 / 19 / 11 = 30 | 30 | 0 (1 open-by-design set: R7 build-time re-checks) |
| 03 | 2026-11-07 | Completeness / coverage vs §23 exit gates | 0 / 2 / 8 / 5 = 15 | 15 | 0 (3 suggestions logged, not patches) |
| 04 | 2026-11-07 | Traceability / contradictions | 0 / 0 / 6 / 7 = 13 | 13 | 0 (2 suggestions logged) |
| 05 | 2026-11-07 | Security / safety / control authority | 0 / 8 / 17 / 5 = 30 | 30 | 4 (need design decisions — see below) |
| 06 | 2026-11-07 | Testing / verification / reproducibility | 0 / 1 / 17 / 7 = 25 | 25 | 0 (2 of the 4 iter-5 deferrals shared) |
| 07 | 2026-11-07 | Data / contracts / interfaces | 0 / 3 / 5 / 2 = 10 | 10 | 0 |
| 08 | 2026-11-07 | Dependencies / sequencing | 0 / 3 / 5 / 2 = 10 | 10 | 0 |
| 09 | 2026-11-07 | Research freshness / versions | 0 / 0 / 4 / 4 = 8 | 6 | 2 (other-phase, applied in final assembly) |
| 10 | 2026-11-07 | §24 vertical-slice walkthrough + final robustness | 0 / 0 / 1 / 4 = 5 | 5 | 0 |
| **Total** | | | **0 / 24 / 95 / 51 = 170** | **167** | **3 (+4 design-decision deferrals)** |

> **Correction (final review, 2026-11-07):** the total row previously read
> 0 / 24 / 100 / 46 = 170. That was an arithmetic error in the total row
> only — the per-iteration rows above were and are correct, and they sum
> to **0 Critical / 24 High / 95 Medium / 51 Low = 170**. The "100
> Medium, 46 Low" figure quoted from this row in the build handoff is
> therefore wrong and must not be reused.

Subtask totals: build pass **336** → after iterations **381**
(+45: P2 +1, P3 +1, P4 +5, P5 +3, P6 +4, P7 +11, P8 +9, P9 +11).
Per-phase final: P0 30, P1 32, P2 31, P3 31, P4 37, P5 33, P6 34,
P7 43, P8 39, P9 41, P10 30.

## Iteration 01 — Build pass

- 24 items (0/7/13/4). The four phase builders recorded the issues
  they found while drafting, including: D4 gated before v0.1.0 but
  still OPEN (P10.19); §2.1 omitting the §22.3 control-authority
  qualification (P10.15); D1/D3/D5 open vs §22.3 promotion rule (P9);
  OpenEMS API-key-vs-mTLS trap (P6.07); Wasmtime CVSS 9.9 escapes and
  ≥50.0.0 pin; ANDES GPLv3 vs Apache-2.0 (P8); PyTorch/PyG skew;
  secondary-only React/TS/Vite versions; NATS CVE attribution and
  Jepsen durability finding.
- Deferred: 1 — report TOC cosmetic (§0 unnumbered in TOC). Report is
  out of ledger scope; noted for a future report patch.

## Iteration 02 — Deep re-analysis

- 30 items (0/0/19/11). Re-read report sections for Phases 0–3 and
  re-verified RESEARCH.md versions against primary sources.
- Headline: CVE-2026-33222 attribution was wrong (fixed in 2.11.15 /
  2.12.6, not 2.15.2; the 2.15.2 item is CVE-2026-111943) — corrected
  in P1/P2 RESEARCH.md. NATS CNCF-exit story superseded (NATS
  graduated in CNCF 2 Nov 2026). Foundations §6.4 had no carrier —
  new P2.31 (versioned telemetry corrections + as-of replay).
  P3.12 consumed "forecasts from Phase 2" that no phase produced —
  new P3.31 (synthetic forecast fixture). Foundations §6.2 SOC
  divergence and §7.7/§7.8 feasibility extras recorded (P0.07, P0.12,
  P0.26). OIDC stub un-gated (P1.17), cargo-deny license gate added
  (P1.27).
- Deferred: none as "fix later". R7 build-time re-checks remain open
  by design (utoipa/tracing/tower/OTel versions, FERC ISO dates,
  Axum 0.8.9 — secondary only; asserting more would violate AGENTS.md
  rule 1).

## Iteration 03 — Completeness vs §23 exit gates

- 15 items (0/2/8/5) + 2 checked-not-gaps. Highs: I3-01 alarm model
  undefined anywhere (§20.1 requires "key alarms" in Overview) —
  new P5.31; I3-02 §22.3 requires *all* command-state transitions
  tested, P4.10 tested only a representative set — new P4.33
  (exhaustive 14×14 matrix). Also added: P4.34 unbalanced-model
  deferral, P4.35 feasibility runtime, P4.36 audit-events API,
  P4.37 RETURN-loop contract, P5.32 tenant display, P5.33 live-update
  ordering/dedup, P6.31 time sync, P6.32 decommissioning, P6.33 gate
  evidence pack, P6.34 secure-boot position.
- Suggestions deferred (logged, not patches): adapter lifecycle
  management (belongs Phase 7), screen-reader check (not required by
  §20.2), OpenEMS EVSE/load lab coverage (depends on D5).

## Iteration 04 — Traceability / contradictions

- 13 items (0/0/6/7). Verified by script + hand-check: every §-ref in
  all 11 ledgers resolves to a real report heading; every P-ref in
  P4–P6 resolves; zero duplicate titles. Fixed: P5 trace-summary
  off-by-one (3 rows), P4 §14.3 row, P4 cross-phase P5 ranges, P4.15
  "EXPIRED is terminal except for FALLBACK" contradiction, P6.25
  console-cannot-enable (vs P6.14 signed-config gate), P6.07 TLS pin
  depending on a D6 record P6.03 never produced, P6.23 "plugin escape"
  title with no plugin host in Phase 6, P6.24 tracing §19.2 layer 10
  (HIL — Stage 9) for a lab run.

## Iteration 05 — Security / safety / control authority

- 30 items (0/8/17/5). No Critical: nothing grants field dispatch,
  certifies anything, or contradicts the simulation-first ladder.
  All 8 Highs are *missing proofs*, now owning subtasks: P7.33 audit
  integrity, P7.34 plugin-host trust boundary + mTLS, P7.35 ADR-006
  no-dylib CI gate, P7.37 §17.5 invariant map, P7.38 revocation /
  staged rollout, P8.31 tenant isolation, P8.32 model-artifact
  signing, P9.31 hostile replay/forgery test, P9.32 audit integrity +
  edge buffer. Most serious single finding: the P7 CHECKLIST claimed
  "no dylib loading" on evidence that did not exist (now P7.35).
- Deferred (need design decisions the report does not supply):
  I5-S2 signing-key compromise procedure; I5-S3 secret-handle
  expiry/rotation semantics.

## Iteration 06 — Testing / verification / reproducibility

- 25 items (0/1/17/7). Single High: I6-M8 — P9 adapter conformance
  suites named in P9.04 but never executed in pilot configuration
  (negation of §2.3) — new P9.37. Added: P7.41–P7.43 (manifest
  fuzz/property tests, plugin-path seeded replay, scenario-regression
  definition), P8.37–P8.39 (unit/property tests, training-run
  reproducibility + GPU-nondeterminism disclosure, HELICS/ANDES
  cross-checks), P9.38–P9.41 (pilot shadow replay, load test,
  HIL→FAT traceability, scenario-regression re-run). Verified Stage 8
  exit gate per module; HELICS recorded as the one defensible
  exclusion (coordinator, not decision module).
- Deferred: I6-S1 broker-latency bound (needs P7.26's first
  measurement); I6-S2 manifest-schema migration tests (manifest is
  v1-only).
- Process note (owned): the reviewer's patch script double-applied 7
  amendments; caught in verification, duplicates removed, step
  numbering re-verified sequential.

## Iteration 07 — Data / contracts / interfaces

- 10 items (0/3/5/2). Highs patched in Phase 10: P10.02 rewritten
  (OpenAPI diff/breaking check, §15.2 coverage, forbidden
  generic-execute endpoint, 26-event NATS vocabulary, §10.1
  14-entity coverage, units/sign, time fields); P10.18 added to
  Dependencies of 13 subtasks (contract freeze gates the release);
  P10.04 per-data-class §16.3 retention; P10.06 `ESTIMATED` flagged
  in API responses, not only console.

## Iteration 08 — Dependencies / sequencing

- 10 items (0/3/5/2). Highs patched in Phase 10: P10.11/P10.12
  (outbox/inbox survive restore and upgrade/rollback without loss or
  duplicate publication); P10.13 (no silent breaking Protobuf/REST/
  event-subject change); P10.26/P10.27 dependency and decision-pack
  corrections. D4/D6 wording fixed (D4 final closure for the v1.0.0
  artifact set; D6 before any read-only pilot).

## Iteration 09 — Research freshness / versions

- 8 items (0/0/4/4). Re-verified tool versions (Syft 1.56.0, Grype
  0.121.0, Trivy 0.76.1, OSV-Scanner 2.7.0, cdxgen 13.5.0), Trivy
  CVE-2026-33634 incident detail, EU CRA date corrected to
  11 December 2027, CycloneDX 1.7 / SPDX 3.1 RC noted; new §8
  re-verification log in P10 RESEARCH.md.
- Deferred (other-phase, out of patch scope): M-9.3 P7 Wasmtime CVE
  ranges; M-9.4 P1/P2 NATS v2.15.2 date + sibling CVE-2026-33223.
  **Both applied in the final assembly pass** (see below).

## Iteration 10 — §24 walkthrough + final robustness

- 5 items (0/0/1/4). Walked all 10 §24 steps across Phases 0–5:
  every step owned by a named subtask with testable acceptance
  (mapping table in `iteration-10/findings.md`). Whole-ledger sweep:
  0 orphan references, no Dependencies field points to a later
  phase, no fake dates/versions/certification claims in P10.
  P10.09 now requires the full §24 slice on the release candidate
  before replay; P10.16 date-gated on the NIST Rev. 4 comment period
  (closes 30 Nov 2026).
- **Correction to this iteration's claim:** it reported "332 subtasks
  total confirmed" — that was the pre-review build count and is
  stale. Verified total after all patches is **381** (build 336 + 45
  added). The build pass itself created 336 subtasks, not 332 as
  stated in one handoff (30+32+30+30+32+30+30+32+30+30+30 = 336).

## Final assembly pass (parent, 7 Nov 2026 — not a numbered iteration)

- Applied the two iteration-09 deferred Mediums: M-9.3 (P7
  RESEARCH.md — CVE-2026-115889 fixed on main in 43.0.0, so
  43.0.0–47.x affected only by 115893; added GHSA-m9f7-9w4p-gv38)
  and M-9.4 (P1/P2 RESEARCH.md — NATS v2.15.2 published 26 Oct 2026,
  not 5 Nov; added sibling CVE-2026-33223).
- Wrote `ledger/README.md`, `ledger/MASTER_LEDGER.md`, and this log.
- Ran `python tools/verify_docs.py` and `python tools/validate_agent.py`
  (see parent report for actual output) and `git status` (only
  untracked `ledger/` added; nothing committed or pushed).

## Final review (2026-11-07 — closing entry, independent reviewer)

- Re-counted the ledger from scratch (grep/Python): 381 subtasks
  (30/32/31/31/37/33/34/43/39/41/30), 381 unique contiguous heading IDs,
  667 unchecked / 0 checked boxes, 0 orphan P-references, 0 unresolvable
  §-references. Read MASTER_LEDGER.md in full and Phase 4 end-to-end.
- **Found and fixed 8 errors (0 Critical):** (1) this log's total row read
  0/24/100/46 — per-iteration rows sum to 0/24/95/51, corrected above with
  a note (the "100 Medium, 46 Low" figure in the build handoff is wrong);
  (2) P7 RESEARCH.md §3 said Rust 1.99.0 was current — 1.99.1 superseded
  it 15 Oct 2026; (3) P9 ledger intro said `P9.01 … P9.30` for a 41-subtask
  phase; (4–7) handover Dependencies narrower than Inputs in P2.29, P3.30,
  P4.32, P5.30 — iteration 3's patch log claimed P4.32/P5.30 were widened;
  the files did not reflect it until now; (8) PostgreSQL 18.6 upgraded
  from secondary-only to primary-confirmed in P1/P2 RESEARCH.md.
- Research spot-checked against primary sources (10 claims): Rust 1.99.1 /
  1.100, NATS 2.15.2 + CVE-2026-33222/33223, Wasmtime CVEs, pandapower
  3.5.6, OpenEMS 2026.11.0, FERC 2222 ISO-NE 1 Nov 2026, HiGHS 1.15.1,
  PostgreSQL 18.6 — all confirmed except the stale Rust row above.
- Verified exit gates (§23), release mapping (§2), §24 slice ownership,
  D1–D6 mapping, control authority, solver discipline, and certification
  language; no contradictions found.
- **Ledger v1.0 FINALIZED.** Record: `ledger/FINALIZATION.md` (also
  summarised at the top of `MASTER_LEDGER.md`). Known limitations recorded
  there: checkbox granularity differs between P0–P2 (per-criterion) and
  P3–P10 (per-subtask); research is a 7 Nov 2026 snapshot. Remaining open
  items are decisions/work, not ledger defects — D1–D3 first.

## Remaining open items

- **Critical: 0. High: 0 unowned** — all 24 Highs found now have
  owning subtasks; they are open *work* (proofs to execute before
  their phase's exit gate can honestly pass), not unrecorded gaps.
  No iteration 11 was needed: the task's rule was to keep iterating
  only while Critical/High items remain open *without an owner*, and
  none do.
- Open by design (re-check at build time, recorded in the owning
  phase): React/TypeScript/Vite versions, OpenADR 3.1 certification,
  SPP/CAISO FERC 2222 dates, PyTorch/PyG skew, RLlib Ray version,
  OpenDSS/GridLAB-D licenses (D4), IEEE 1547 revision date, D1–D6
  themselves.
- Deferred design decisions (I5-S2/S3, I6-S1/S2, I3-S1/S2/S3, report
  TOC cosmetic) — see `MASTER_LEDGER.md` §8.
