<!-- P10.22 — Site integration test harness — packaged and runnable (packaging
     record). PACKAGE DELIVERED AS MANIFEST + ENTRY PROCEDURE ONLY — never a
     built binary. HONEST STATEMENT (binding): the harness has NEVER been
     packaged-built and NEVER been run; "runnable" is verified only by
     CONSTRUCTION (every entry command names suite files and runner
     subcommands that exist in the repository) until the first human
     execution by a non-author fills the run record. The clean-environment
     run, the non-author execution, and the version-mismatch detection are
     HUMAN GATES — all EMPTY. -->
# P10.22 — Site Integration Test Harness — Packaged and Runnable (Packaging Record — never built, never run)

Date: 2026-11-14. Subtask: P10.22 (`ledger_phase_10.md` §P10.22). Owner
role: Integration engineering (with platform engineering). Report trace:
§25 (pilot release: "site integration test harness"); §23 Stage 9. Inputs:
P10.01 scope freeze (`01_release_scope_freeze.md`); the Phase 9 lab/field
test procedures (P9.14, P9.16–P9.20) as the harness's test content;
P9.37 conformance and P9.41 regression legs (task-assigned additions,
cited by path); P10.18 candidate (`<PENDING P10.18>` — no candidate build
exists). ENTRY_NOTE boundaries 1 and 4 apply.

## 1. Honest status — the plain statement

**The harness is packaged (manifest + entry procedure + run-record
template) and has never been packaged-built and never been run.** There is
no harness binary and no harness script — the packaging is deliberately
**config/manifest files + documented entry commands** over suites that
already exist in the repository, because no binary may be built in this
loop and no run may be executed. "Runnable" is therefore verified **only
by construction**: every entry command in `harness-manifest-v1.json` names
suite files and runner subcommands that exist in the repository at the
pinned paths (verified by reading on 2026-11-14), and the entry procedure
in `README.md` composes them in the order the source records mandate.
Whether a site integrator can actually execute the package end-to-end in a
clean environment is the human gate (ledger steps 3, acceptance criteria
(a)–(d)) — **all four acceptance criteria are UNMET and not claimed**:

- (a) clean-environment run by a non-author: **NO RUN EXISTS**;
- (b) every check produces pass/fail, never a silent skip: the SEMANTICS
  are pre-registered (`run-record-template.md`, manifest `pass_semantics`);
  no result exists to demonstrate them in execution;
- (c) version-mismatch detection: the REFUSAL RULE is written (README §4);
  the recorded refusal from an actual mismatched deployment does not exist;
- (d) harness versioned with the release: `site-harness-v1` is recorded;
  the release-candidate identifier cell is `<PENDING P10.18>` — the
  version binding is recorded as a position, not as a verified tie to a
  built candidate.

## 2. The harness composition (which suites constitute the site harness)

Seven legs; every one cited by path, none restated or re-decided:

| Leg | Content source (cited by path) | Role in the site harness |
|---|---|---|
| A | `ledger/phase-09-pilot-qualification/14_lab_integration_test.md` (L1–L10, F11–F18) | the P9.14 lab cases: read-only telemetry end-to-end, command-path-disabled negatives, fault injection |
| B | `ledger/phase-09-pilot-qualification/37_adapter_conformance_pilot.md`; suite `flexcore-edge-adapter-conformance-suite-v1` over `crates/edge-agent/tests/conformance_suite_p6_19.rs` (+ `conformance_p6_13.rs`, `openems_p6_10.rs`, `openems_e2e_p6_11.rs`, `security_p6_23.rs`) | the P9.37 conformance leg |
| C | `ledger/phase-09-pilot-qualification/41_scenario_regression_pilot.md` §2 legs L-1..L-5 (`scenarios/synthetic-portfolio/**`, `scenarios/library/` lib-01..lib-08, `plugins/scenarios/sdk-*.json`, the cited suite files) | the P9.41 regression legs, pinned seeds/tolerances carried BY CITATION |
| D | `ledger/phase-09-pilot-qualification/18_telemetry_clock_verification.md` (+ `crates/edge-agent/tests/clock_quality_p6_31.rs`) | the P9.18 telemetry/clock checks |
| E | `ledger/phase-09-pilot-qualification/19_command_dry_run.md` | the P9.19 command-lifecycle dry run in shadow |
| F | `ledger/phase-09-pilot-qualification/20_fallback_drills.md` | the P9.20 fallback drills (gated on the filled §4 register) |
| G | `ledger/phase-09-pilot-qualification/16_field_acceptance_plan.md` | the P9.16 field-acceptance site leg (never a lab leg) |

The manifest pins this composition with per-leg entry commands,
preconditions, and the no-seed-override / no-tolerance-edit rules inherited
verbatim from the source records.

## 3. The packaging spec — what "packaged and runnable" means here

| Requirement (ledger step 2) | How this package meets it | Status |
|---|---|---|
| Versioned with the release | `flexcore-site-integration-harness-v1` / `site-harness-v1`; `release_candidate_id` cell pinned-but-EMPTY until P10.18 resolves; the harness version and location go to P10.26 | recorded; candidate tie PENDING |
| README stating prerequisites, how to run, how to read output | `harness/site-integration/README.md` §3–§6 | written |
| Runnable by a site integrator, not just the authors | entry procedure + manifest are self-contained; every command copyable; no author-only knowledge required by the text — **execution proof is the human gate** | by construction only |
| Version-mismatch refusal (ledger step 4) | README §4: refuse (never warn-and-continue) on any deployed version differing from the pins or unpinned; procedural enforcement until a compiled driver exists — recorded honestly | rule written; detection UNPROVEN |
| Not a built binary | the package ships JSON + Markdown only; a compiled driver would be a new harness version with its own review | satisfied by design |

## 4. The run record — EMPTY

The run record is `harness/site-integration/run-record-template.md`:
configuration block, per-leg/per-check result table (every cell
`(EMPTY — UNRUN)`), applicability record, failures-and-dispositions table,
verbatim suite-id capture, and the automatic-FAIL audit block. It is filled
ONLY from actual run output the same day as the run. No copy of it has ever
been filled; there is no clean-environment run record because no run
happened.

## 5. What "runnable" does and does not mean (the honest statement, unpacked)

- "Runnable by construction" means: the commands resolve to real files and
  runner subcommands, the procedure is ordered and gated the way the source
  records require, and the pass/fail semantics are pre-registered so they
  cannot be fitted to results afterwards.
- It does NOT mean: the package has ever executed, that a non-author has
  ever run it, that any check has ever passed, or that the harness works.
  Zero observed failures and zero observed passes are both true and NEITHER
  is claimed.
- A passing harness run would establish only what README §7 states —
  never conformance/certification (P9.37 §7 wording governs any claim),
  never site behaviour (leg G owns it), never safety, never field write
  authority.

## 6. Findings (recorded, not fixed)

- **F-P10.22-1:** the release-candidate identifier does not exist
  (`<PENDING P10.18>`); the manifest's `release_candidate_id` is
  deliberately an unfilled placeholder — a run started today would be a
  run against an unpinned build and is void by the version rule.
- **F-P10.22-2:** the version-mismatch refusal is PROCEDURAL (executor
  step 0), not mechanical — no binary exists to enforce it. The ledger
  acceptance criterion (c) is met only by a RECORDED refusal from an
  actual run. This is the honest gap between "packaged as
  manifest/procedure" and "the harness refuses".
- **F-P10.22-3 (dependency gaps, honest):** no clean environment, no
  staging deployment, no mTLS material, no filled P9.20 §4 register, no
  customer acceptance criteria — the preconditions of README §3 are all
  unmet. Legs F and G are structurally unrunnable today and the manifest
  records their BLOCKED semantics rather than dropping them.
- **F-P10.22-4 (carried):** every constituent leg is WRITTEN-UNRUN
  (P9.14/P9.37/P9.41/P9.18/P9.19/P9.20 all carry that status); the harness
  inherits no run evidence from any of them, and a leg's own unfilled
  record blocks its harness leg by the preconditions rule.

## 7. Reviewer-verification procedure

1. Confirm §1's plain statement stands verbatim and no executed-run claim
   exists anywhere in the package.
2. Confirm every result cell in `run-record-template.md` is
   `(EMPTY — UNRUN)` and the configuration block is unfilled.
3. Traceability check: every manifest `entry` command resolves to real
   repository paths (suite files exist; runner subcommands match the cited
   phase records; the seed/tolerance rules match the source records
   verbatim).
4. Confirm the automatic-FAIL rules and the REFUSED-VERSION-MISMATCH rule
   are intact and that no check can resolve to a silent skip.
5. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the package is written; the harness has never run.

## 8. Filling commands (for the human executor)

```sh
# (a) the documented-tools gate (run after ANY edit to the package):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# (b) the entry procedure IS the run: harness/site-integration/README.md §5,
#     steps 0–7, in order; the manifest's entry commands per leg.
# (c) fill harness/site-integration/run-record-template.md FROM THE RUN
#     OUTPUT ONLY, the same day; record the non-author + clean-environment
#     statements; record any REFUSED-VERSION-MISMATCH verbatim (that is the
#     acceptance-criterion (c) evidence).
# (d) record the harness version + location in the release notes (P10.26)
#     and file the filled record as the P10.22 evidence in CHECKLIST.md.
```

Status: **WRITTEN-UNRUN** — the harness package (manifest + entry procedure
+ run-record template) is written; no packaging build exists to do (none is
permitted — the package is config-only by design); no run has ever happened;
every result cell is EMPTY and stays EMPTY until a human non-author
execution fills it from actual run output.
