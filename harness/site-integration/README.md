<!-- P10.22 — Site integration test harness, PACKAGING (README + manifest + run-record
     template). This directory is the harness PACKAGE: manifest/config files and a
     documented entry procedure only — NEVER a built binary. HONEST STATUS: the harness
     has never been packaged-built and never been run; "runnable" is verified only by
     construction (each entry command cites a suite file that exists in the repository)
     until the first human execution records a run. -->
# FlexCore Site Integration Test Harness — `site-harness-v1`

| Field | Value |
|---|---|
| Harness id | `flexcore-site-integration-harness-v1` |
| Harness version | `site-harness-v1` (versioned with the v1.0.0 release candidate; the candidate identifier cell is EMPTY — see `harness-manifest-v1.json` `release_candidate_id`) |
| Subtask | P10.22 (`ledger/phase-10-v1-release-hardening/ledger_phase_10.md` §P10.22) |
| Decision record | `ledger/phase-10-v1-release-hardening/22_site_harness.md` |
| Control status | **Simulation / read-only and shadow by design.** This harness exercises NO field write authority. The command-path cases assert refusal-before-issue; any command reaching an adapter or device is an automatic FAIL of the whole run. FlexCore is a supervisory orchestration layer — no protection-grade, hard-real-time, or sub-cycle control. |
| Build status | **WRITTEN, NEVER PACKAGED-BUILT, NEVER RUN.** Every result cell in `run-record-template.md` is EMPTY. "Runnable" below is verified by CONSTRUCTION only: every entry command names a suite file or record that exists in the repository at the manifest's pinned paths. No clean-environment run by a non-author has happened. Nothing here is evidence of a run. |

## 1. What this harness is

The P10.22 deliverable is the site integration test harness as a **shippable,
runnable package** (§25 pilot release): a site integrator — not just the
harness's authors — must be able to execute it against a staged deployment and
read per-check pass/fail output. In this repository the package is:

- `harness-manifest-v1.json` — the machine-readable manifest: which suites
  constitute the harness, each cited by repository path, with its entry
  command, its evidence rule, and its pass/fail semantics;
- this `README.md` — the entry procedure: prerequisites, execution order, how
  to read output, and the version-mismatch refusal rule;
- `run-record-template.md` — the EMPTY run record the executor fills **from
  actual run output only**.

There is **no harness binary and no harness script**. The harness is a
composition of suites that already exist in the repository, driven by the
runners those suites already define. Packaging means: the manifest pins the
composition and the entry commands so the composition is stable, reviewable,
and executable by someone who did not write it. If a later phase adds a
compiled driver, that is a new harness version with its own review — never a
silent addition to this one.

## 2. What the harness contains (the composition)

Seven legs; every leg is cited by path in the manifest with its per-check
identity. All of them are **WRITTEN-UNRUN** as of packaging (2026-11-14) —
the harness packages their definitions; it does not inherit any run result
from them, because none exists.

| Leg | Content source (cited by path) | Checks |
|---|---|---|
| A — Lab integration checks | `ledger/phase-09-pilot-qualification/14_lab_integration_test.md` §3 (L1–L10 telemetry end-to-end; L8–L10 command-path-disabled; F11–F18 fault injection) | 18 checks against a staged deployment; negative command cases require a security witness |
| B — Adapter conformance | `ledger/phase-09-pilot-qualification/37_adapter_conformance_pilot.md` §2; suite id `flexcore-edge-adapter-conformance-suite-v1` (`crates/edge-agent/tests/conformance_suite_p6_19.rs`, `conformance_p6_13.rs`, `openems_p6_10.rs`, `openems_e2e_p6_11.rs`, `security_p6_23.rs`) | 11 suite cases + the 9-case fault-injection set (`crates/edge-agent/src/conformance.rs`) |
| C — Scenario-regression legs | `ledger/phase-09-pilot-qualification/41_scenario_regression_pilot.md` §2, legs L-1..L-5 (`scenarios/synthetic-portfolio/`, `crates/flexcore-application/tests/fault_injection.rs`, `shadow_evaluation.rs`, `crates/flexcore-sim-runner` phase-3 scenarios + `scenarios/library/` lib-01..lib-08, the 21-file Phase 6 edge register, `plugins/scenarios/sdk-*.json` via `crates/flexcore-kernel/tests/scenario_regression_p7_43.rs`) | per-leg pre-registered expectations, pinned seeds (master seed 20260601 — no override), pinned tolerances |
| D — Telemetry/clock checks | `ledger/phase-09-pilot-qualification/18_telemetry_clock_verification.md` (procedures §2–§4) + `crates/edge-agent/tests/clock_quality_p6_31.rs` | §10.3/§10.4 six-state quality mapping, hold-on-drift clock discipline |
| E — Command-lifecycle dry run (shadow) | `ledger/phase-09-pilot-qualification/19_command_dry_run.md` (procedure + §8 EMPTY record) | propose → validate → feasibility → approve → hold-before-issue in shadow, evidence shown at each gate |
| F — Fallback drills | `ledger/phase-09-pilot-qualification/20_fallback_drills.md` (drill plan; §4 register must be FILLED with real operator review first) | link-loss buffering, central outage, adapter failure; no drill runs against an EMPTY §4 register |
| G — Field acceptance (site leg) | `ledger/phase-09-pilot-qualification/16_field_acceptance_plan.md` FAT catalogue §6 (pre-registered; numeric thresholds registered by its X-8 amendment block BEFORE the run) | the site/customer acceptance cases — executed only at a real site, never in the lab |

Composition rule: a leg runs only when its source record's own preconditions
are met (e.g. leg F refuses to run while the P9.20 §4 fallback-policy register
is unfilled; leg G is not a lab leg at all). A leg that cannot run is recorded
`NOT-APPLICABLE-with-reason` or `BLOCKED-<reason>` in the run record — never
silently skipped.

## 3. Prerequisites (what must exist before any run)

1. **The release candidate.** The P10.18 candidate identifier and its built
   artifacts. Today the cell is `<PENDING P10.18>` — no run can start, and no
   run against an unpinned build would be a harness run at all.
2. **A staging deployment** mirroring the target site composition: control
   plane (PostgreSQL, NATS, API, console), edge-agent build at the pinned git
   SHA, a site-EMS instance at the re-pinned installed version, and the
   mapping profile (shipped `crates/edge-agent/config/openems-mapping-profile.v1.json`
   or a pilot variant created additively). Deployment procedure: the
   deployment/upgrade model document (`docs/operations/deployment-upgrade-model.md`,
   P10.24 — DESIGN only until a review deployment fills it).
3. **Toolchain** (pre-installed by the site engineer; the harness installs
   NOTHING): Rust toolchain per `rust-toolchain.toml` (1.99.1) for the cargo
   test legs; Python 3.11+ for the leg L-3 worker environment
   (`apps/worker/python/requirements.txt`, pandapower 3.5.6 — installed and
   verified by the human executor, never assumed).
4. **mTLS material** for any end-to-end leg: root CA, central certificate,
   edge credential issued via the enrollment flow (P9.33). No PKI ships with
   the harness.
5. **A security witness** for every negative command case (legs A/E).
6. **A filled P9.20 §4 fallback-policy register with real operator review**
   before leg F, and customer acceptance criteria before leg G.

## 4. Version-mismatch rule (refuse, never warn-and-continue)

The manifest pins `release_candidate_id` (EMPTY — filled at packaging-time
resolution of P10.18). Before each leg, the executor records the DEPLOYED
versions of every component in the run record's configuration block. The
rule:

- If any deployed component version differs from the pinned candidate
  identifiers (git SHAs, artifact hashes, EMS version, profile version), the
  leg is **refused**: it is recorded `REFUSED-VERSION-MISMATCH` with the
  observed-vs-pinned pair, and the leg is not executed. A pass is never
  recorded against the wrong build.
- An unpinned component (no recorded candidate value for it) is a mismatch
  too: the harness run may not start until every component it exercises is
  pinned.
- Because this package is manifest + procedure (no binary), the enforcement
  is **procedural** (the executor's step 0 in §5) until a compiled driver
  implements it mechanically — recorded honestly as such; a future harness
  version that adds the driver makes the refusal mechanical. The acceptance
  criterion "the harness detects a version mismatch" is met only by a
  RECORDED refusal from an actual run — never by this rule's existence alone.

## 5. Entry procedure (the site engineer's steps, in order)

```sh
# 0. Version gate (procedural refusal — §4): read every deployed component's
#    reported version; fill the run-record configuration block; compare
#    against the manifest pins. ANY mismatch → record REFUSED-VERSION-MISMATCH
#    for the affected legs and STOP.

# 1. Documented-tools gate (repository checks; not a harness leg):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# 2. Legs B and C (in-repo suites, deterministic, no deployment needed):
cargo test -p flexcore-edge-agent --test conformance_suite_p6_19 -- --nocapture
cargo test -p flexcore-edge-agent --test conformance_p6_13 -- --nocapture
cargo test -p flexcore-edge-agent --test openems_p6_10 --test openems_e2e_p6_11 -- --nocapture
cargo test -p flexcore-edge-agent --test security_p6_23 -- --nocapture
cargo test -p flexcore-sim-runner --test phase3_scenarios -- --nocapture
cargo test -p flexcore-application --test fault_injection -- --nocapture
cargo test -p flexcore-application --test shadow_evaluation -- --nocapture
cargo test -p flexcore-kernel --test scenario_regression_p7_43 -- --nocapture
#    ...plus the manifest's per-leg commands (legs L-1/L-3 sim-runner runs,
#    the marked Phase 6 register files). A suite failure stops the run and
#    is dispositioned before any end-to-end leg.

# 3. Stand up the staging deployment (prerequisite 2); pin versions;
#    fill the configuration block COMPLETELY before the first end-to-end case.

# 4. Legs A, D, E (end-to-end, in order: telemetry first, then the negative
#    command cases under the security witness, then the shadow dry run):
#    drive the L/F/D cases of 14_lab_integration_test.md against the RUNNING
#    deployment; execute 18_telemetry_clock_verification.md §2–§4;
#    execute 19_command_dry_run.md and hold before issue.

# 5. Leg F (only when the P9.20 §4 register is filled and reviewed):
#    execute 20_fallback_drills.md drills; witness present.

# 6. Record EVERY result into run-record-template.md FROM THE ACTUAL OUTPUT
#    the same day. Filling any cell from anything else is fabrication.

# 7. Leg G is out of scope for a staging run — it executes only at a real
#    site per 16_field_acceptance_plan.md; record BLOCKED-NOT-A-LAB-LEG here.
```

## 6. How to read the output

- Each check resolves to exactly one of **PASS** (observed behaviour matches
  the pre-registered expectation, evidenced by a named artifact — log,
  capture, audit extract), **FAIL**, **NOT-APPLICABLE** (written reason tied
  to the environment; never "covered by unit tests"), or
  **BLOCKED-`<reason>`** (a recorded prerequisite is missing — a blocked
  check is unexecuted, not passed).
- **Automatic FAIL of the whole run**, regardless of which check produced
  it: (i) any command reaching an adapter or device in any leg; (ii) any
  fault producing a SILENT WRONG VALUE (a value delivered as fresh/GOOD that
  is not, or a refusal with no audit record); (iii) any check closed without
  a result or a reasoned not-applicable.
- A failed check is dispositioned exactly one of: fixed-and-re-run /
  accepted limitation filed to the limitations register / blocks the release
  (P10.27). A failure without a disposition blocks the release gate.
- Every suite that prints a suite id (e.g. `flexcore-edge-adapter-...
  -conformance-suite-v1`, `flexcore-plugin-sdk-scenario-regression-v1`) is
  quoted verbatim into the run record — the id, not a paraphrase, is what a
  conformance-wording claim may cite.

## 7. What a passing harness run would and would not establish

A recorded all-PASS run establishes ONLY that the named builds, on the
recorded versions, in the recorded staging environment, on the recorded date,
produced the pre-registered outcomes with evidence. It does NOT establish:
site behaviour (a staging environment is not the site — leg G owns site
acceptance); conformance or certification of any protocol or adapter (the
only permitted wording is the P9.37 §7 claim boundary); performance at pilot
scale (P9.39 owns it); safety or protection-grade properties; field write
authority of any kind; or correctness of the platform (regression-free legs
compare against pre-registered expectations, and the command path stays
refusal-asserted — nothing in this harness ever issues a field command).

## 8. Reviewer-verification procedure

1. Confirm this README and the manifest make NO executed-run claim and that
   `run-record-template.md` has every result cell EMPTY.
2. Trace every manifest `entry` command to real repository paths (suite
   files exist; runner subcommands match the cited phase records).
3. Confirm §4's refusal rule and §6's automatic-FAIL rules are intact and
   that no check can resolve to a silent skip.
4. Confirm the composition matches §2 (seven legs, sources cited by path,
   none restated or re-decided by this package).
5. Confirm the ledger tick wording is "implemented, awaiting human
   build/test" — the package is written; the harness has never run.
6. For a filled future run record: every result traces to a resolving
   artifact; every version cell names the INSTALLED version; the executor
   is not an author of the harness; the clean-environment claim is backed
   by the recorded environment description.
