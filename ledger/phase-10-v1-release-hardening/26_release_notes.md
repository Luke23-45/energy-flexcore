<!-- P10.26 — Release notes and known limitations. HONEST RECORD: the
     deliverable is the ADDITIVE DRAFT
     docs/releases/v1.0.0-release-notes-draft.md (version
     v1.0.0-draft) plus this record. The notes are a DRAFT — NOT a
     release, NOT published, NOT reviewed by product or security
     (both sign-offs UNSIGNED). The known-limitations section carries
     the ACTUAL registers: the P9.27 register (LR-001..LR-211 + 42
     carried pointers, version pilot-limitations-register-v0.1-draft)
     item-for-item BY CITATION plus every Phase 10 limitation; the
     known-issues section cites the recorded findings (F-P10.07-5
     clock-median defect, F-P10.04-1..3, F-P10.05-1..3, F-P10.06-1..3,
     F-P10.07-1..4, F-P10.08-1..4, the P8 F-1 HIGH, and the
     P10.19/P10.20/P10.21 closure findings). The AGENTS.md §3
     no-safety/no-certification/no-protection-grade discipline and the
     report §0.3 non-promises are quoted VERBATIM; the §19.3
     NOT-TESTED scale/capability statements are carried. WRITTEN-UNRUN:
     no claim in the notes exceeds what its cited record states. -->
# P10.26 — Release Notes and Known Limitations (DRAFT v1.0.0-draft — not published, not reviewed; every capability claim traced to an EMPTY/UNRUN source cell)

Date: 2026-11-14. Subtask: P10.26 (`ledger_phase_10.md` §P10.26).
Owner role: Product (with security). Report trace: §2.1 (v1.0.0
definition); §25 (pilot release).
Deliverables:
1. **`docs/releases/v1.0.0-release-notes-draft.md`** — the additive
   release-notes DRAFT artifact (new file; version `v1.0.0-draft`; no
   existing document modified).
2. This record — the traceability map (§2–§4), the P9.27 carry-forward
   reconciliation procedure and its findings (§5), and the unsigned
   review block (§6).

Inputs (all cited, none restated): P10.01 scope
(`01_release_scope_freeze.md`); P10.10 benchmark pack
(`10_benchmark_results_pack.md` — EMPTY); P10.22 harness
(`22_site_harness.md`); P9.27 limitations/residual-risk register
(`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md`
— MUST be carried forward complete); P10.16/P10.17
(`16_cyber_risk_consolidation.md`, `17_operational_risk_review.md`);
P10.25 (`25_security_consolidation.md` — VERDICT NOT APPROVED);
P10.15 (`15_operator_mode_qualification.md` — NOT QUALIFIED);
P10.19/P10.20/P10.21 closure records; the Phase 10 test-suite finding
records (`04_`, `05_`, `06_`, `07_`, `08_`).

## 1. Honest status — the plain statement

**The release notes exist as a DRAFT, and that is all they are.** The
version is `v1.0.0-draft`; the document states on its face that it is
not a release announcement and that nothing is published. Concretely:

- **No release candidate exists** (P10.18 UNRUN), so the notes describe
  positions and EMPTY verification cells — never results.
- **The notes are NOT reviewed.** The ledger step-5 review by product
  and security is a human gate; both sign-off cells in the draft's §9
  and in §6 here are UNSIGNED.
- **The known-limitations section is the strongest section, filled
  from the ACTUAL carried registers** — the P9.27 register carried
  item-for-item by citation (LR-001…LR-211 + 42 carried pointers) plus
  every Phase 10 limitation. No item is dropped or softened; the
  register's own rule (never paraphrase into softer language) governs
  the human review.
- **Every factual claim in the draft is traced to a source record**
  (§2–§4 here). Where a source cell is EMPTY/UNRUN/UNSIGNED, the draft
  says so rather than upgrading it — e.g. the operator-approved control
  status of the ladder row is presented as a target position with the
  NOT QUALIFIED verdict of P10.15 cited alongside.

Ticking P10.26 in the ledger means "implemented, awaiting human
review/sign-off" — the same honest convention as every prior phase.

## 2. Draft-section traceability — what v1.0.0 contains (ledger step 1)

| Draft section (of `docs/releases/v1.0.0-release-notes-draft.md`) | Claim made | Source record (path) | Source's own state |
|---|---|---|---|
| §1 release-ladder row (verbatim) + capability-state table | What v1.0.0 IS, per §2.1, with each of the five §25 capabilities' actual state | `01_release_scope_freeze.md` §2/§2.1; `23_`/`09_`/`10_`/`24_`/`25_`/`22_` records | Ladder row verbatim; every capability state cited from its record (walkthrough UNRUN, benchmark EMPTY, doc PLACEHOLDER values, review NOT APPROVED, harness never run) |
| §2 control status | Simulation/shadow only is what is qualified; operator-approved NOT QUALIFIED; bounded automation post-1.0, not enabled | `15_operator_mode_qualification.md` (NOT QUALIFIED); `ledger/phase-09-pilot-qualification/30_phase_exit_gate_record.md` §1/§5; `01_release_scope_freeze.md` §2 consequence | Verdicts carried unsoftened; "does not authorize operator-approved control at any site" quoted in substance from the freeze record |
| §3 release contents | Scope/adapters, frozen contracts, benchmark summary, deployment/upgrade model, harness, support matrix, artifacts | `01_`/`02_`/`10_`/`24_`/`22_`/`14_`/`19_`/`21_` records | Every "contains" row states its EMPTY verification (UNVERIFIED adapter cells; UNRUN Buf verdicts; EMPTY benchmark pack; PLACEHOLDER site values; zero support cells; no SBOM/artifact/signature) |
| §4 known limitations | The carried registers, complete | P9.27 register + `16_`/`17_`/`19_`/`20_`/`21_` | §5 below |
| §5 known issues | The recorded findings, cited | `07_`/`04_`/`05_`/`06_`/`08_`/`19_`/`20_`/`21_` + `ledger/phase-08-advanced-coordination/25_security_review.md` | §6 below |
| §6 NOT-TESTED statements | What nothing establishes | `10_benchmark_results_pack.md` §7 (§19.3 scope statement); ENTRY_NOTE boundary 3; `18_`/`14_` records | §7 below |
| §7 wording discipline | No-safety/no-certification/no-protection-grade verbatim; §0.3 non-promises verbatim; solver-class rule | `AGENTS.md` §3 rules 2–4; report §0.3 | Verbatim quotations |
| §8 issue intake | Pointer to the P10.30 intake and P10.20 §8 process | `20_vuln_scan_closure.md` §8 | Channels UNDECIDED (`21_signed_artifacts.md` §8) — stated, not invented |

## 3. The control-authority position in the notes (ledger step 4) — exact wording discipline

The draft's §2 carries the position the ledger step 4 requires, in
three sentences backed by three records:

1. **Operator-approved control by default** is the release-ladder row
   (§2.1, verbatim in the draft §1) — presented with the recorded
   Stage 9 dependency: `15_operator_mode_qualification.md` verdict
   **NOT QUALIFIED** and `30_phase_exit_gate_record.md` §5
   **NOT MET — prepared, not demonstrated**.
2. **Bounded automation is post-1.0**, per deployment, site, and asset
   class, after separate qualification (§21.4), and **not enabled** —
   no automation envelope exists in the release configuration
   (P10.15 §6–§7; the P10.30 deferral record carries it forward).
3. **The release does not authorize field write authority on these
   records** — the freeze record's own consequence sentence is carried:
   a v1.0.0 published on the recorded gate exceptions would ship the
   artifacts of a pilot-ready release WITHOUT the qualification
   evidence the label claims (`01_release_scope_freeze.md` §2).

## 4. What the benchmark section of the notes claims — nothing (P10.10 dependency)

The draft prints NO benchmark number, threshold, tolerance, or summary
value, because `10_benchmark_results_pack.md` has zero filled cells and
the X-8 pre-registration ordering (thresholds registered BEFORE the
runs, dated and signed) has not been executed. When the human gate
fills the pack, the notes' benchmark section is filled FROM THE PACK
ONLY (pack §9 step f hands it to P10.26) with the release-candidate
identifier — never from a record summary.

## 5. The P9.27 carry-forward (ledger step 2) — item-for-item procedure and findings

The ledger acceptance (a): "every P9.27 register item appears in the
known-limitations section or is closed with evidence". The draft's §4
carries the register **item for item through citation**: the register
file
(`ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md`,
version `pilot-limitations-register-v0.1-draft`) IS the itemised list;
the draft's §4 theme groups are its reading order with the LR ranges
named, and the draft states explicitly that the register is the
authoritative itemised list the themes point into. The reconciliation
procedure at the human review:

1. For every LR-id LR-001…LR-211 (the on-disk version) confirm the
   register row is either (i) still OPEN and represented in the draft's
   §4 by its theme group + LR range citation, or (ii) closed with
   evidence — no evidence of closure exists today, so the expected
   result is (i) for all.
2. Confirm the 42 carried pointers (Phase 6/7/8 O-registers, register
   §4) are represented — the draft's §4 carries them through the
   register citation and the supply-chain/edge-security rows.
3. Confirm the Phase 10 additions are present: benchmark emptiness
   (P10.09/P10.10), zero verified browser/OS cells (P10.14), no SBOM
   (P10.19), zero cleared scans (P10.20), nothing signed (P10.21),
   harness never run (P10.22), no candidate (P10.18), and the D6
   scope-addition blocker (P10.16 §6).
4. Any dropped or softened item is a release-notes finding owned by
   this subtask's author; the reviewer records it and fixes the DRAFT
   (never the register).

**Findings carried (recorded honestly, same as P10.16 C-5 / P10.17
O-4):** the register file on disk numbers **LR-001…LR-211** (211 traced
+ 42 carried pointers) while P10.01 §10 cites "LR-001..LR-225". These
notes cite the register AS IT IS; reconciling the count is the register
owner's DATED AMENDMENT (the register's §7 versioning rule), never an
edit made in this wave. The draft records this as KN-0 in its §4.

## 6. Known-issues section — the recorded findings, as cited in the draft's §5

The draft's known-issues table cites (never fixes; `crates/**` and the
source registers are outside this subtask's ownership):

- **F-P10.07-5** — the clock-median defect in the EXISTING P6.31 unit
  test (`the_tolerance_boundary_is_respected_inclusively`,
  `crates/edge-agent/src/clock_quality.rs` ~703–712): the assertion
  would FAIL on the first human run; operationally relevant because
  clock quality gates command expiry; fix options recorded for the
  file's owner (`07_edge_offline_tests.md` §6; carried as R-7 in
  `17_operational_risk_review.md` §3 and as a CI-failure expectation in
  `18_ci_gate_run_record.md` §2).
- **F-P10.04-1..3** (`04_authz_tenant_tests.md` §4) — the `Action::ALL`
  absence, the behavioural grant-matrix assertion, the 401/403
  distinction constraint.
- **F-P10.05-1..3** (`05_optimizer_validation.md` §6) — the closed
  §12.6 vocabulary (unbounded → `numerical_failure` with verbatim
  status text), the HiGHS warning-text nuance, the test-grade config
  digest constraint.
- **F-P10.06-1..3** (`06_telemetry_handling_tests.md` §6) — the
  `age > limit` staleness boundary vs clock-skew separation, the
  invalidate-on-any-transition envelope rule, the `Degraded` →
  `Unsynchronized` mapping.
- **F-P10.07-1..4** (`07_edge_offline_tests.md` §6) — at-least-once
  redelivery, audited drop-oldest with the audit-journal sizing
  obligation, journal encryption at rest OPEN, keyed-checksum command
  signature (tamper-EVIDENCE, not a guarantee).
- **F-P10.08-1..4** (`08_plugin_boundary_tests.md` §6) — the
  audit-completeness gap and the recorded plugin constraints.
- **P8 F-1 (HIGH)** — the unguarded `torch.load(...)` at the GNN
  artifact load: OPEN, blocks the with-framework activation, unreachable
  today (`ledger/phase-08-advanced-coordination/25_security_review.md`
  §2 F-1, via `16_cyber_risk_consolidation.md` §7.3).
- **P10.19 F-1..F-7, P10.20 F-1..F-5, P10.21 F-1..F-6** — the closure
  records' own findings (no SBOM/scan/signature; zero versions cleared;
  channels and support commitment UNDECIDED).

## 7. The §0.3 non-promises and NOT-TESTED statements in the draft (ledger steps 3 and the §19.3 discipline)

- The draft's §7 quotes `AGENTS.md` §3 rules 2–3 **verbatim** (no
  safety or certification claims; no protection-grade claims), the
  report §0.3 non-promises list **verbatim** (hard real-time /
  protection-grade; primary frequency response / synthetic inertia;
  autonomous grid switching; universal AC-OPF global optimality;
  universal market qualification / FERC Order 2222; certified protocol
  conformance from adapter implementation alone; safe field dispatch
  without site-specific acceptance; GNN/MARL safe-by-construction),
  and the solver-class rule (AGENTS.md §3 rule 4) in substance.
- The draft's §6 carries the NOT-TESTED statements: no benchmark result
  of any kind; no executed suite (zero observed failures never claimed
  from an unrun suite — ENTRY_NOTE boundary 3); no CI run, no candidate;
  no performance/latency figure claimable; no tested browser/OS cell;
  no scalability claim supported by evidence; and the §19.3 scale
  statement (first distribution-feeder tests do not establish
  capability for a 10,000-bus utility network — carried from
  `10_benchmark_results_pack.md` §7).

## 8. Harness and deployment-model locations recorded (ledger inputs)

- Site integration test harness: `harness/site-integration/` (P10.22
  record; manifest + entry procedure + EMPTY run-record template;
  never packaged-built or run) — the harness version is recorded in
  the notes at packaging time, versioned with the release.
- Deployment/upgrade model: `docs/operations/deployment-upgrade-model.md`
  (P10.24, additive; design-not-deployed; every site value
  PLACEHOLDER).

## 9. Review and sign-off block — UNSIGNED

| Item | Value |
|---|---|
| Product review of the notes (accuracy; every claim traced) | `<EMPTY — UNSIGNED>` |
| Security review of the notes (limitations carried; no softened wording) | `<EMPTY — UNSIGNED>` |
| Item-for-item reconciliation result (P9.27 register ↔ notes §4) | `<EMPTY — procedure recorded, execution at review>` |
| Notes review verdict | **NOT REVIEWED** (the draft exists; the ledger step-5 review has not happened) |
| Product signature | **UNSIGNED — `<EMPTY>`** |
| Security signature | **UNSIGNED — `<EMPTY>`** |

## 10. Findings recorded by this record

| # | Finding | Disposition |
|---|---|---|
| N-1 | The release notes are a DRAFT (v1.0.0-draft), additive, unpublished, and unreviewed; both sign-offs UNSIGNED | Binding on this record |
| N-2 | The P9.27 register is carried item-for-item through citation; the draft names the register as the authoritative itemised list; the reviewer reconciliation procedure is §5 steps 1–4 | Executed at the human review |
| N-3 | LR count discrepancy (LR-211 on disk vs LR-225 cited in P10.01 §10) carried into the notes as KN-0; register owner's dated amendment | Not reconciled here |
| N-4 | No benchmark number appears in the notes — the P10.10 pack is EMPTY and the X-8 ordering is unexecuted; the section fills only from the executed pack | §4 |
| N-5 | The control-authority position is carried with its NOT QUALIFIED / NOT MET verdicts adjacent to the ladder row, so the notes cannot be read as qualifying the mode | §3 |
| N-6 | No existing document was modified; `docs/releases/v1.0.0-release-notes-draft.md` is a new file | Additive rule held |

## 11. Reviewer-verification procedure (human)

1. Confirm the draft states DRAFT / `v1.0.0-draft` on its face and
   nowhere claims publication, a built candidate, or a run result.
2. Spot-check five rows of the draft's §1–§3 against the cited source
   records: the claim must not exceed the source's own state.
3. Run the §5 reconciliation procedure against the on-disk P9.27
   register; record the result and any finding in §9's reconciliation
   cell — fixes go to the DRAFT, never to the register.
4. Confirm the draft's §5 known-issues table cites each finding's
   source path faithfully (especially F-P10.07-5 and P8 F-1) and that
   no fix was applied to `crates/**` or any source register.
5. Confirm §7's verbatim quotations match `AGENTS.md` §3 rules 2–3 and
   report §0.3 exactly, and §6's NOT-TESTED statements are present.
6. Confirm `git status --porcelain` shows no modification outside this
   subtask's ownership (the two new files + this record + the ledger
   tick).
7. Confirm the ledger tick reads "implemented, awaiting human
   review/sign-off".

## 12. Filling commands (human, at execution time)

```sh
# At the release-notes review (ledger step 5):
#   1. Run the §5 reconciliation; fill §9's reconciliation cell.
#   2. Fill the benchmark summary section ONLY from the executed
#      P10.10 pack (pack §9 step f), with the release-candidate ID.
#   3. Add any accepted vulnerabilities (P10.20 signed, time-bounded)
#      and removed browser/OS cells (P10.14) as new limitation rows.
#   4. Record the harness version (from the packaging step) in the
#      notes; version the notes with the release.
#   5. Sign product + security (§9) — last, never before the review.

# Item-for-item check aids:
grep -c "^| LR-" ledger/phase-09-pilot-qualification/27_limitations_residual_risk.md
grep -n "LR-225" ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md

# Repository verification after any edit to these files:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 13. Status

**WRITTEN-UNRUN / DRAFT.** The release-notes draft exists as an
additive artifact with every claim traced to its source cell; the
product/security review, the P9.27 reconciliation execution, the
benchmark-section fill, and both signatures are the human gates and
have not happened. Nothing is published; P10.28 stays blocked on the
P10.27 decision, which is NOT MADE.
