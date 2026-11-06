# Finalization Record — FlexCore Implementation Ledger v1.0

- **Status:** FINALIZED
- **Ledger version:** v1.0
- **Date:** 2026-11-07
- **Scope:** 11 phases (Phase 0–10), 381 subtasks, 69 files, ~1.6 MB under `ledger/`
- **What "finalized" means:** the ledger as a *plan* is complete, internally
  consistent, and traceable to the report. It does **not** mean any
  implementation work is done — all 381 subtasks are unchecked and no code
  exists. Do not read this record as a build sign-off.

## Verified totals (re-counted by the final reviewer, not trusted from the build report)

| Phase | Subtasks | Phase | Subtasks |
|---|---|---|---|
| 0 Product freeze | 30 | 6 Edge + protocol | 34 |
| 1 Rust foundation | 32 | 7 Plugin SDK | 43 |
| 2 Asset + telemetry | 31 | 8 Advanced coordination | 39 |
| 3 Flexibility + scheduling | 31 | 9 Pilot qualification | 41 |
| 4 Grid feasibility + dispatch | 37 | 10 v1.0.0 release hardening | 30 |
| 5 Console + simulator | 33 | **Total** | **381** |

Counting method: "Acceptance criteria" lines per `ledger_phase_<N>.md`
(30/32/31/31/37/33/34/43/39/41/30 = 381); subtask heading IDs are unique
and contiguous 1..N within every phase (381 unique IDs); 667 unchecked
boxes and 0 checked boxes across the phase ledgers; 0 orphan subtask
cross-references (every `P<N>.<mm>` mentioned is a defined heading);
0 unresolvable `§` references (every `§N` / `§N.M` resolves to a real
report heading). File count: 69 (MASTER_LEDGER.md, README.md,
FINALIZATION.md, 11 × 5 phase files, 10 iteration findings + ITERATION_LOG.md).

## Review performed

1. **Iterations 1–10** (2026-11-07, audit trail in `ledger/_iterations/`):
   **170 findings — 0 Critical / 24 High / 95 Medium / 51 Low**; 167
   patched, 3 deferred (+ 4 design-decision deferrals). All 24 Highs have
   owning subtasks; they are open *work*, not unrecorded gaps.
   > Correction to the record: the iteration log's total row previously read
   > 0 / 24 / 100 / 46. The per-iteration rows sum to **95 Medium / 51 Low**;
   > the total row was an arithmetic error, fixed in the final review. The
   > "100 Medium, 46 Low" figure in the build handoff is wrong.
2. **Final independent review** (2026-11-07, reviewer did not build the
   ledger): re-counted everything above with grep/Python; read
   MASTER_LEDGER.md in full and Phase 4 end-to-end; read Phase 0/1/2/3/5/
   6/7/8/9/10 headers, trace summaries, and exit-gate sections; checked
   exit gates against report §23, release mapping against §2, the §24
   10-step slice ownership, D1–D6 mapping, control-authority wording,
   solver-class discipline, and certification language; spot-checked 10
   RESEARCH.md version/status claims against primary sources (below).

## Research spot-checks (final review, 7 Nov 2026)

| Claim | Verdict |
|---|---|
| Rust 1.99.1 stable 15 Oct 2026; 1.100 due 12 Nov 2026 | Confirmed (Rust blog / releases.rs). P7 RESEARCH.md said 1.99.0 was current — **stale, fixed** (F2 below). |
| NATS 2.15.2 published 26 Oct 2026 | Confirmed (Debian tracker). |
| CVE-2026-33222 / CVE-2026-33223 fixed in 2.11.15 / 2.12.6 | Confirmed (NVD/Debian CVE text). |
| Wasmtime CVE-2026-115889 fixed 36.0.19 / 43.0.0; CVE-2026-115893 fixed 36.0.19 / 48.0.7 / 49.0.4; 50.0.0 carries both fixes | Confirmed. |
| pandapower 3.5.6, 7 Oct 2026 | Confirmed (PyPI). |
| OpenEMS 2026.11.0, 31 Oct 2026 | Confirmed (GitHub releases / Zenodo). |
| FERC Order 2222, ISO-NE live 1 Nov 2026 | Confirmed (ISO-NE, 2 Nov 2026). |
| HiGHS / highspy 1.15.1, 2 Jul 2026 | Confirmed (PyPI / HiGHS releases). |
| PostgreSQL 18.6, 13 Aug 2026; 18.5 never shipped | Confirmed (postgresql.org announcement). P1/P2 RESEARCH.md carried it as secondary-only — **upgraded to primary-confirmed** (F8 below). |
| NIST SP 800-82 Rev. 4 draft, comments close 30 Nov 2026 | Not re-checked in this pass; carried as stated in P10 with its date gate (P10.16). Re-check at implementation time. |

## Errors found and fixed in the final review (no Critical found)

| # | Severity | Finding | Fix (file) |
|---|---|---|---|
| F1 | Medium | Iteration-log total row read 0 / 24 / 100 / 46 = 170; per-iteration rows sum to 95 Medium / 51 Low. | `_iterations/ITERATION_LOG.md` total row corrected to 0 / 24 / 95 / 51, with a correction note. |
| F2 | Medium | P7 RESEARCH.md §3 said "Current stable Rust 1.99.0" — superseded by 1.99.1 on 15 Oct 2026, three weeks before the cut-off. | `phase-07-plugin-sdk/RESEARCH.md` corrected to 1.99.1 with a dated correction note. |
| F3 | Low | P9 ledger intro said subtask IDs run `P9.01 … P9.30`; the phase has 41 subtasks (11 added in iterations). | `phase-09-pilot-qualification/ledger_phase_9.md` corrected to `P9.01 … P9.41`. |
| F4 | Low | P2.29 (exit-gate review) Inputs include P2.31 but Dependencies read P2.01–P2.28 only. | `phase-02-asset-telemetry/ledger_phase_2.md`: Dependencies → P2.01–P2.28, P2.31. |
| F5 | Low | P3.30 (handover) Inputs include P3.31 but Dependencies read P3.01–P3.29 only. | `phase-03-flexibility-scheduling/ledger_phase_3.md`: Dependencies → P3.01–P3.29, P3.31. |
| F6 | Low | P4.32 (handover) Inputs read P4.01–P4.37 but Dependencies read P4.01–P4.31. Iteration 3's patch log claimed this range was widened — it was not in the file. | `phase-04-grid-feasibility-dispatch/ledger_phase_4.md`: Dependencies → P4.01–P4.37. |
| F7 | Low | P5.30 (exit-gate demo + handover) Inputs read P5.01–P5.33 but Dependencies read P5.01–P5.29. Same unapplied iteration-3 patch-log claim as F6. | `phase-05-operator-console-simulator/ledger_phase_5.md`: Dependencies → P5.01–P5.33. |
| F8 | Low | PostgreSQL 18.6 carried as secondary-only in P1/P2 RESEARCH.md. | Both rows upgraded to primary-confirmed (postgresql.org announcement, 13 Aug 2026); build-time `SELECT version()` kept as a sanity check. |

Two apparent duplicates found and **cleared, not fixed**: the heading-ID
check first flagged P6.29 and P7.31 as duplicated — both were artefacts of
the check itself (a heading line whose *title* mentions another phase's ID,
e.g. `## P7.01 — … P6.29 handover`). True heading IDs are unique.

## Sign-off checklist (each item checked with evidence, not asserted)

- [x] Subtask total re-counted: 381, per-phase counts match MASTER_LEDGER.md §1 and README.md.
- [x] Every subtask has an ID, report trace, inputs, steps, deliverable, testable acceptance criteria, owner role, dependencies, verification method, risks (spot-read Phase 4 end-to-end; all 381 have an "Acceptance criteria" line).
- [x] Phase exit gates match report §23 Stages 0–9 (MASTER_LEDGER.md §3 restates each; verified against the report text).
- [x] Release mapping matches report §2.1 (P0 pre-v0.1.0, P1–P2 v0.1.0, P3 v0.2.0, P4–P5 v0.3.0, P6 v0.4.0, P7 v0.5.0, P8 v1.x/v2.x research, P9–P10 v1.0.0).
- [x] §24 10-step vertical slice owned by named subtasks (MASTER_LEDGER.md §6; iteration 10 walked all 10 steps).
- [x] D1–D6 all OPEN, mapped to owning subtasks, none silently decided; recommended defaults labelled as recommendations (MASTER_LEDGER.md §5).
- [x] Control authority stated per phase: 0.x simulation-only / simulation-shadow / research-only; operator-approved control only at v1.0.0 (P9/P10); bounded automation post-1.0 only. No subtask grants field dispatch early.
- [x] No safety/certification/protection-grade claims: all 12 "certified" hits in phase ledgers are negations or references to local certified controllers; solver classes stated correctly throughout (HiGHS LP/QP/MIP, Clarabel conic not MISOCP, AC-PF ≠ AC-OPF, IPOPT local).
- [x] Research snapshot is 7 Nov 2026; 10 claims spot-checked above, 1 stale claim fixed (F2), 1 secondary claim upgraded to primary (F8); remaining unverified items stay labelled ASSUMPTION / open verification tasks.
- [x] Repo checks pass (outputs below); `git status` shows only untracked `ledger/`; nothing in `docs/foundations/*` or the report was touched; nothing committed or pushed.

## Verification (actual output, 2026-11-07, run from repo root)

```
$ python3 tools/verify_docs.py
report: docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md
lines: 2241  sections: 28  headings: 127  tables: 32
OK: headings, section references, fences, tables, references, TOC   (exit 0)
$ python3 tools/validate_agent.py
skills: 5  roles: 5
OK: skills, roles, and README index are valid   (exit 0)
$ git status --short
?? ledger/
```

Note: `python` is not on PATH in this VM; `python3` was used.

## Known limitations (do not mistake these for gaps)

- **Checkbox granularity is inconsistent across phases.** P0/P1/P2 use one
  checkbox per acceptance criterion (~4 per subtask); P3–P10 use one
  checkbox per subtask with prose acceptance criteria. The criteria are
  still testable in both styles, but per-criterion auditability is weaker
  in P3–P10. Converting 172 subtasks was judged higher-risk than the
  benefit; implementers should evidence each prose criterion separately
  when ticking a subtask box.
- **Research is a 7 Nov 2026 snapshot.** Rust 1.100 (12 Nov 2026), the
  NIST SP 800-82 Rev. 4 comment close (30 Nov 2026), PostgreSQL 18.7
  (due 12 Nov 2026), pandapower 4.0.0, and FERC 2222 RTO go-lives all move
  after the cut-off. Re-verify at implementation time; do not pin from memory.
- **Iteration-3 patch-log claim vs file state (F6/F7).** The iteration-3
  findings claim the P4.32/P5.30 dependency ranges were widened; the files
  did not reflect that until this final review fixed them. Patch logs are
  claims, not proof — future iterations should re-grep the patched lines.

## Remaining open items (what honestly prevents calling this "complete")

These are decisions and work, not ledger defects. The ledger is finalized;
the *project* is not started.

1. **Phase 0, D1–D3 — the blocking user decision:** target jurisdiction/
   market (D1), first portfolio/service (D2), network class/model fidelity
   (D3). Each needs an owner, decision venue, and date (or the decision
   itself) before engineering can honestly proceed; §22.3 forbids promotion
   beyond read-only until D1/D3/D5 close for the deployment.
2. **D4** (license/dependency audit, before v0.1.0; re-closed for v1.0.0 in
   P10.19) and **D6** (cyber/OT boundary review, before any read-only pilot)
   and **D5** (first protocol + asset mix, before v0.4.0) — all OPEN.
3. **All 24 Highs from iterations 1–10** are open *work* with owning
   subtasks (proofs/tests to execute before their phase's exit gate can
   honestly pass). The most load-bearing: P5.31 (alarm contract) and P4.33
   (exhaustive 14×14 command-transition matrix).
4. **Deferred design decisions** needing choices the report does not supply:
   signing-key compromise procedure; secret-handle expiry/rotation (I5-S2/S3);
   broker-latency bound (needs P7.26's first measurement); manifest-schema
   migration (I6-S1/S2); adapter lifecycle, screen-reader check, OpenEMS
   EVSE/load lab coverage (I3-S1/S2/S3).
5. **Open verification tasks (re-check at build time):** React/TypeScript/
   Vite versions; OpenADR 3.1 certification profiles; SPP/CAISO FERC 2222
   dates; PyTorch 2.15 vs PyG 2.8.0 skew; RLlib Ray version; OpenDSS/
   GridLAB-D licenses (D4 audit); IEEE 1547 revision publication date;
   NIST SP 800-82 Rev. 4 final text.
