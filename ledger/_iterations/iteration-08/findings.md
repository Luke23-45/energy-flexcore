# Iteration 8 findings — Phase ordering and inter-phase dependencies

Reviewer: iterations 7–10 subagent. Date: 7 November 2026.
Scope: `ledger/phase-10-v1-release-hardening/` (patched) against all 11 phase ledgers,
`.agent/DECISIONS.md` (D1–D6, all OPEN), report §22.2–§22.3, §25.2.
Method: scripted parse of every phase ledger's `Dependencies:` fields and every `P?.??`
reference (0 orphan references across the whole ledger); read P9 header and P9.01–P9.06,
P9.12, P9.19, P9.30, P0.06, P1.28, P6.01–P6.03; compared P10 header/PLAN §6/CONTEXT §6
wording against `.agent/DECISIONS.md` and the P9 header.

Severity scale: Critical = a phase depends on a later phase's output, or a gate is
sequenced so the release could pass it; High = a dependency or gate is stated in prose
only and missing from the machine-checkable `Dependencies:` field, or a gate is weakened
in wording; Medium = a dependency is missing from one subtask's field while present
elsewhere; Low = cosmetic inconsistency or an acceptable transitive dependency.

Counts: **High 3, Medium 5, Low 2. Critical 0.**

## (a) Missing subtasks

None. All prerequisites exist in earlier phases; no new subtask was needed. P10 subtask
total stays **30**.

## (b) Issues / errors / contradictions

### H-8.1 — Candidate identity stated in prose only; missing from 13 subtasks' Dependencies fields
- **Severity: High.**
- **File + subtasks:** `ledger/phase-10-v1-release-hardening/ledger_phase_10.md`,
  **P10.03, P10.04, P10.05, P10.06, P10.07, P10.08, P10.09, P10.11, P10.12, P10.14,
  P10.15, P10.17, P10.23**.
- P10.18 states "Blocks P10.03–P10.17 (candidate identity)" and PLAN §6 step 2 says the
  candidate "is the artifact every verification subtask tests" — but only P10.16 named
  P10.18 in its Dependencies field. The other thirteen gate-verification subtasks depended
  on P10.01 only. A reader (or a future dependency checker) following only the Dependencies
  fields could run a gate against an unidentified build and still claim the field was
  satisfied. PLAN §6 already named the risk ("Candidate drift (verifications run against
  different builds)") without the field-level enforcement.
- **Patch applied:** P10.18 added to the Dependencies field of all thirteen subtasks, each
  annotated "(release candidate)". P10.10 left as-is: it depends on P10.09, which now names
  P10.18 — transitive and recorded as L-8.1, not patched. PLAN §6 step 3 now states the
  field-level rule explicitly. No cycle is introduced: P10.18 depends only on P10.01/P10.02/
  P10.13, and none of the thirteen feeds back into it.

### H-8.2 — D6 gate weakened in the P10 header
- **Severity: High.**
- **File + subtask:** `ledger_phase_10.md`, header (lines 41–44 pre-patch); also
  `CONTEXT.md` §6 and `PLAN.md` §2.
- The header said D1–D3, D5, D6 "must be closed … before the deployment is promoted beyond
  read-only mode". `.agent/DECISIONS.md` gates D6 **before any read-only pilot** — a
  strictly earlier gate. The P9 header states the stricter gate correctly ("D6 must close
  before any read-only pilot"); P10 contradicted it. Under the P10 wording a read-only
  pilot could start with D6 open.
- **Patch applied:** header rewritten with the D6-before-any-read-only-pilot gate; CONTEXT.md
  §6 D6 row and PLAN.md §2 D1/D2/D3/D5/D6 row corrected to the same wording.

### H-8.3 — D4 "closes here" unqualified, contradicting the before-v0.1.0 gate
- **Severity: High.**
- **File + subtask:** `ledger_phase_10.md`, header; also `CONTEXT.md` §6 and `PLAN.md` §2.
- The header said D4 "closes here, P10.19". `.agent/DECISIONS.md` gates D4 **before
  v0.1.0**; P9.06/P10.19 is the final closure for the v1.0.0 artifact set, not the first
  D4 closure. The unqualified wording reads as if D4 were first closed in Phase 10, which
  contradicts the P9 header ("D4 was gated before v0.1.0; its final closure for the v1.0.0
  release artifact set is a Phase 10 subtask (P10.19), not a Phase 9 subtask").
- **Patch applied:** header, CONTEXT.md §6 D4 row, and PLAN.md §2 D4 row now say "final
  closure for the v1.0.0 release artifact set … first gated before v0.1.0".

### M-8.1 — P10.26 Dependencies omit four subtasks its own steps name as limitation sources
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.26**.
- P10.26 step 2 (browser/OS cells removed) draws on P10.14, step 4 (control-authority
  position) on P10.15, and the release-notes limitations draw on the D4/SBOM limits
  (P10.19) and accepted vulnerabilities (P10.20) — none of the four appeared in its
  Dependencies field (P10.01, P10.10, P10.16, P10.17, P10.22, P10.25; P9.27).
- **Patch applied:** Dependencies extended with P10.14, P10.15, P10.19, P10.20, each
  annotated with why.

### M-8.2 — P10.17 omits P10.11 (candidate restore evidence)
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.17**.
- The operational risk review consolidates backup/restore evidence, but its Dependencies
  field named P10.12 and P10.23 only — not the restore test P10.11 whose result it
  consumes.
- **Patch applied:** P10.11 added to P10.17 Dependencies (alongside P10.18, see H-8.1).

### M-8.3 — P10.27 decision-pack enumeration omits P10.21–P10.24
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.27**.
- P10.27 depends on P10.01–P10.26, but its step-1 decision-pack enumeration listed
  P10.01–P10.17, P10.18, P10.19, P10.20, P10.25, P10.26, and P9.30 — silently dropping
  the signed artifacts/support matrix (P10.21), the site integration test harness (P10.22),
  the operator-workflow stability check (P10.23), and the deployment/upgrade guide (P10.24)
  from the pack a go decision is made on.
- **Patch applied:** step 1 enumeration extended with P10.21–P10.24, each named.

### M-8.4 — P10.18 CI "license/security review" gate reads as the D4 closure
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.18**.
- P10.18's CI gate list (§22.2) includes "license/security review", while P10.19 — which
  depends on P10.18 — is the legal D4 determination. Nothing stopped a reader from
  recording the CI gate as D4 closed and treating P10.19 as a formality.
- **Patch applied:** P10.18 Risks now states the §22.2 gate is only the automated scan and
  "P10.18 must not be recorded as D4 closed".

### M-8.5 — P10.16 NIST Rev. 4 re-check has no date gate (comment period closes 30 Nov 2026)
- **Severity: Medium.**
- **File + subtask:** `ledger_phase_10.md`, **P10.16**.
- Step 3 said "the comment period closed 30 November 2026" as if it were already past. As
  of the 7 November 2026 review date it is in the future, so the check cannot return a
  "final" answer yet — and nothing in Dependencies or sequencing said so. A go decision
  sought before 30 November 2026 could silently assume the final matches the draft.
- **Patch applied:** step 3 date-gated — if run before 30 November 2026, record draft status
  plus a dated re-check; a pre-30-November go decision must carry that re-check as a
  P10.27 condition.

### L-8.1 — P10.10 candidate identity is transitive only (via P10.09)
- **Severity: Low.**
- **File + subtask:** `ledger_phase_10.md`, **P10.10**.
- P10.10 depends on P10.09, which now names P10.18. Transitive and unambiguous; no patch.
  Recorded so a later reviewer does not "fix" it by adding a redundant direct edge.

### L-8.2 — §22.3 gate-9 mapping: header said P10.11–P10.12, CHECKLIST said P10.11–P10.13
- **Severity: Low.**
- **File:** `ledger_phase_10.md` header vs `CHECKLIST.md` Gate 9.
- The header's §22.3 mapping listed item 9 as P10.11–P10.12; CHECKLIST Gate 9 evidence cites
  P10.11–P10.13. P10.13 (schema migration compatibility) is part of the backup/restore/
  upgrade gate family, so the CHECKLIST is the correct one.
- **Patch applied:** header mapping corrected to P10.11–P10.13. CHECKLIST.md itself was not
  edited (it was already correct).

## (c) Suggestions

- A future dependency checker should treat a `Dependencies:` field as the complete
  prerequisite list and flag any subtask whose steps name another subtask that its field
  does not — that rule would have caught M-8.1, M-8.2, and M-8.3 mechanically.
- The D1–D6 gate table in `.agent/DECISIONS.md` is the single source for gate timing; phase
  headers should quote it rather than paraphrase it (the H-8.2/H-8.3 paraphrases are what
  drifted).

## (d) Stale research

None in iteration-8 scope (sequencing, not versions). Versioned claims are iteration 9.

## Checked and clean (re-verified, no finding)

- **No phase assumes a later phase's output.** Every `Dependencies:` field in all 11 phase
  ledgers points to the same or an earlier phase. Forward-phase references that exist are
  benign mentions, not dependencies: P3 names P4.12 (a schedule revision rejected in
  Phase 4), P5.12, P5.13–P5.20 (handover/consumed-by notes); P4 names P5.11, P5.13–P5.20;
  P9 names P10.19 (D4 final closure is Phase 10, not Phase 9) and P10.11/P10.26
  (feeds/carries forward). None is a prerequisite.
- **D1–D6 closers exist and are sequenced:** P0.06 records D4/D5/D6 as scheduled gates (D4
  before v0.1.0, D5 before v0.4.0, D6 before any read-only pilot); P1.28 is D4 inventory
  input (D4 stays OPEN); P6.01 closes/records D5; P6.02 is D4 follow-through for the Phase 6
  stack; P6.03 is the D6 cyber/OT boundary review for the pilot site; P9.03 traces D3,
  P9.04 traces D5, P9.06 is "OT/IT boundary review — D6 closure", P9.12 is interconnection
  and authority engagement (§22.3 market/interconnection qualification, IEEE 1547-2018 as
  adopted locally).
- **Phase 10 as release gate, not duplicated Phase 9 work:** P10 re-verifies Phase 9 outputs
  on the release candidate (P10.03–P10.17) and closes the release artifact set (P10.18–
  P10.30). No P10 subtask re-builds a Phase 9 deliverable; cross-references to P9
  (P9.01, .03, .04, .06–.08, .10, .12, .14–.27, .30) all resolve — verified by script.
- **Orphan references: 0** across the whole ledger (every `P?.??` reference resolves to a
  subtask heading in its phase file).

## Applied vs deferred log

| Item | Disposition |
|---|---|
| H-8.1 | **Applied** — P10.18 added to Dependencies of P10.03–P10.09, P10.11, P10.12, P10.14, P10.15, P10.17, P10.23; PLAN §6 step 3 states the rule |
| H-8.2 | **Applied** — ledger header, CONTEXT.md §6 D6 row, PLAN.md §2 D6 wording |
| H-8.3 | **Applied** — ledger header, CONTEXT.md §6 D4 row, PLAN.md §2 D4 row |
| M-8.1 | **Applied** — P10.26 Dependencies +P10.14/P10.15/P10.19/P10.20 |
| M-8.2 | **Applied** — P10.17 Dependencies +P10.11 |
| M-8.3 | **Applied** — P10.27 step 1 enumeration +P10.21–P10.24 |
| M-8.4 | **Applied** — P10.18 Risks clarification |
| M-8.5 | **Applied** — P10.16 step 3 date gate |
| L-8.1 | **Deferred (deliberately, no patch)** — transitive dependency via P10.09 is sufficient; a direct edge would be redundant |
| L-8.2 | **Applied** — header §22.3 item-9 mapping corrected to P10.11–P10.13 |

Deferred: L-8.1 only, by design (see above). No iteration-8 item required an edit outside
phase-10 files.
