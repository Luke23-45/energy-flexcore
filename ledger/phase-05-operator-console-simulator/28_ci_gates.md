# Decision Record — P5.28 (CI quality gates for the console)

- **Date:** 2026-11-12 (Phase 5 wave 6b; loop rules in ENTRY_NOTE.md).
- **Subtask:** P5.28 (§22.2, §7.5, §21.4).
- **Loop status:** NOTHING was installed, built, linted, type-checked, or
  run. No `npm install`, no lockfile, no `node_modules`, no `tsc`/`eslint`/
  `vitest`/`vite` execution, no workflow execution (no `act`, no `gh
  workflow run`). Only files were written: the additive CI job, this
  record, the ledger checkbox, and the HANDOVER row. The first CI run is
  the human's verification point.

## 1. What was implemented

### The `console-quality` job (additive, `.github/workflows/ci.yml`)

Appended at the end of the jobs map, after `buf-breaking`, following the
file's existing comment-block + job style. No existing job was touched.

| Step | Command | Gate |
|---|---|---|
| checkout | `actions/checkout@v4` (same major as every other job) | — |
| Node setup | `actions/setup-node@v4`, `node-version: '22'` | — |
| Install | `npm install` (in `console/`) | install/build failures block |
| Lint | `npm run lint` | failure blocks the change |
| Type-check | `npm run typecheck` | failure blocks the change |
| Tests | `npm run test` | failure blocks the change |
| Build | `npm run build` | failure blocks the change |

The job inherits the workflow's `on: pull_request` + `push: main` triggers
(§22.2: the gates apply to PRs), so a failing lint/type-check/test/build
blocks the merge without any new trigger wiring.

### Script wiring (verbatim from `console/package.json`)

The P5.01 scaffold already defines all four scripts; **no `package.json`
change was needed** and none was made:

| Script | Definition | Note |
|---|---|---|
| `lint` | `eslint .` | flat config already present (`console/eslint.config.js`, pinned typescript-eslint — no new devDep, no new config file needed) |
| `typecheck` | `tsc -b` | the script name is `typecheck` (one word), not `type-check` — CI uses the actual name |
| `test` | `vitest run` | ALREADY non-watch; `npm run test` is used as-is (see decision 2) |
| `build` | `tsc -b && vite build` | emits the static `dist/` bundle (§21.4 build form) |

## 2. Decisions

1. **`npm install`, not `npm ci` (DEFAULT, forced by repo state).** The
   Phase 5 loop's install ban means NO lockfile exists
   (`console/package-lock.json` absent; `npm ci` fails without one). The
   first CI run generates the lockfile; once the human commits it, this
   step should be switched to `npm ci` for reproducible installs. The
   switch point is stated in the job comment.
2. **`npm run test` without `-- --run` (DEFAULT).** The prescribed
   `npm run test -- --run` pattern targets a watch-mode `vitest` script;
   the scaffold's script is already `vitest run`, so appending `--run`
   would be redundant. The non-watch intent is met exactly.
3. **Node 22 LTS (DEFAULT, secondary-source pin).** Node 22 is inside its
   LTS maintenance window as of the writing date. Per the no-hallucinated-
   facts rule this is a DEFAULT, not a verified claim: re-verify the
   current LTS line against nodejs.org at the first run and record any
   change in RESEARCH.md. `cache: npm` is deliberately NOT set —
   setup-node's npm cache errors without a committed lockfile.
4. **Job-level `defaults.run.working-directory: console`.** Keeps the
   script names verbatim instead of `--prefix` rewrites; checkout and
   setup-node are unaffected (they are `uses:` steps).
5. **Rust-side coverage: no change (verified, not modified).** The
   `unit-tests` job already runs `cargo test --workspace`
   (`.github/workflows/ci.yml`), which covers the whole Rust workspace,
   and the console is explicitly NOT a cargo workspace member
   (`01_25_scaffold_live.md` §1). The console addition therefore cannot
   affect any existing Rust job's semantics.
6. **Dependencies satisfied by existence (per task definition).** P5.28's
   ledger dependencies are P5.01 and P5.25; both checkboxes are ticked in
   `ledger_phase_5.md` ("implemented, awaiting human build/test", record
   `01_25_scaffold_live.md`). The live-update correlation work itself is
   NOT part of P5.28.

## 3. Pin honesty

- Every console toolchain version (react 19.3.1, typescript ~7.0.0, vite
  ^8.2.1, @vitejs/plugin-react ^5.0.0, plus the UNPINNED caret ranges for
  vitest/eslint/typescript-eslint/@types/*) is a SECONDARY-SOURCE pin per
  `console/package.json` `x-pin-policy` and RESEARCH.md open task 1.
- The Node pin in the CI job is likewise secondary-source/DEFAULT.
- **The `console-quality` job has NEVER executed** — no local run, no
  workflow dispatch, no CI execution of any kind. Its YAML was written
  and checked for parse-validity only. The first CI run is the human's
  verification point; toolchain corrections (especially the
  typescript-eslint/TypeScript-major peer-cap risk RESEARCH.md flags) may
  follow.

## 4. Honest limitations

- **Nothing executed.** `npm run lint/typecheck/test/build` were never
  run anywhere (local or CI). A green checkbox means "wired", never
  "passing".
- **The first install/build/test pass is the human's gate**, exactly as
  recorded for P5.01/P5.25 in `01_25_scaffold_live.md`.
- **`npm install` in CI is the first-ever install** and will resolve the
  caret ranges (vitest, eslint, typescript-eslint, @types/*) and generate
  the lockfile; the resolved versions must then be recorded in
  RESEARCH.md and the step switched to `npm ci`.

## 5. P5.28 acceptance-criteria coverage (honest gap list)

The ledger's P5.28 exit check names more than this subtask's assigned
scope delivered. Status per item:

| P5.28 item | Status here |
|---|---|
| CI runs lint/type-check/test/build on the console | WIRED (`console-quality` job), never executed |
| Drift test (P5.01 OpenAPI client drift) | NOT wired — the drift test does not exist yet; P5.01 defers the generated client to the first install (`01_25_scaffold_live.md` §1 "Not implemented"). Wiring it is a follow-up once it exists. |
| Scenario reproducibility check in the pipeline | ALREADY PRESENT, not re-wired: the `scenario-regression` job asserts the replay REPORT verdict, and `unit-tests` (`cargo test --workspace`) picks up `apps/sim-runner/tests/reproducibility_p5_19.rs` (both UNRUN per the phase loop rules, but wired) |
| Browser/OS matrix in CI (§21.4) | NOT wired — the level must match what P5.12 recorded, and P5.12's browser-matrix check is a human gate (its record states real AT/browser verification is the human's). Open follow-up. |
| Dependency scanning + SBOM for console deps (§22.2) | NOT wired — the existing `security-and-supply-chain` job covers the Rust side (cargo-deny, cargo-cyclonedx); a console `npm audit`/SBOM step is an open follow-up for the first-install wave, after real dependency versions exist. License re-verification for the D4 audit (React MIT, Vite MIT, TanStack MIT) remains a first-install task per P5.01/P5.28 step 3 — no license claim is asserted here. |
| Deliberate-break verification (P5.28 step 4) | NOT performed — it requires executing CI, which is banned in this loop. The human's first-run procedure: push a commit with a failing test/type error/lint error, observe the job fail, then revert. This is the mitigation for "CI that runs but never fails" and MUST be done before the gate is trusted. |

The ledger checkbox for P5.28 is ticked as "implemented, awaiting human
build/test" — it asserts the wiring, not a passing or even executed gate.

## 6. Verification performed in this loop

- YAML structural parse of the modified `.github/workflows/ci.yml`
  (Python stdlib, no execution of the workflow).
- `python tools/verify_docs.py` and `python tools/validate_agent.py`
  (AGENTS.md §6) — output recorded in the completion report.
