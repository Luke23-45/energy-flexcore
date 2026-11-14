<!-- P10.18 — Full CI gate run and release-candidate build. HONEST RECORD:
     the CI gate run IS A HUMAN GATE and is recorded UNRUN. The
     release-candidate artifact DOES NOT EXIST — the build procedure below
     is described with exact commands and NEVER executed. The run record
     (§5) is an EMPTY template whose result cells are the human gate's
     inputs (CI green = go/no-go input to P10.27, never an agent claim).
     The candidate-identity procedure (§4) is what fills the
     `<PENDING P10.18>` cells in the P10.01 §6.1 freeze. The gate checklist
     (§2) enumerates the ACTUAL job list of .github/workflows/ci.yml — the
     additive release-candidate-build job (§7) is workflow_dispatch-only
     and no existing job was modified. The §22.2 "license/security review"
     CI gate is ONLY the automated scan — D4 closure is P10.19's, and this
     record must never be read as D4 closed. WRITTEN-UNRUN throughout. -->
# P10.18 — Full CI Gate Run and Release-Candidate Build (procedure + EMPTY run record — nothing has been run, nothing has been built)

Date: 2026-11-14. Subtask: P10.18 (`ledger_phase_10.md` §P10.18).
Owner role: Platform engineering. Report trace: §22.2.
Inputs: P10.01 scope freeze
(`ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md` —
candidate identifier `<PENDING P10.18>`); P10.02 contract freeze
(`ledger/phase-10-v1-release-hardening/02_api_contract_freeze.md`); P10.13
migration tests (`ledger/phase-10-v1-release-hardening/
13_schema_migration_compat.md` — concurrent wave-3 sibling, cited by
expected path); the actual job list of `.github/workflows/ci.yml`.

## 1. Honest status — the plain statement

**The full CI gate run is a HUMAN GATE and is recorded UNRUN. The
release-candidate build artifact DOES NOT EXIST.** Nothing in this loop
ran `cargo`, `buf`, `wasm-tools`, `npm`, or any CI job; nothing was
installed; no gate produced a result. What this record delivers:

- the **gate checklist** (§2): every §22.2 gate mapped to the ACTUAL job
  in `.github/workflows/ci.yml` that implements it, with result and
  log-location cells EMPTY — the human run fills them;
- the **release-candidate build procedure** (§3) with exact commands,
  NEVER executed;
- the **candidate-identity procedure** (§4) — the thing that fills the
  `<PENDING P10.18>` cells of the P10.01 §6.1 freeze;
- an **EMPTY run record template** (§5);
- the **additive CI job** (§7) that packages the build+identity steps as a
  workflow_dispatch-only job (no existing job modified).

Ticking P10.18 means "implemented, awaiting human build/test/execution" —
the same honest convention as every prior phase. Per the ENTRY_NOTE
boundary 1, the candidate exists as a described procedure, never a built
artifact.

The ledger's binding rule on this subtask is carried verbatim in
substance: **a gate that is skipped is recorded as skipped with its reason
— a skipped gate is not a passed gate** (§5's template has an explicit
disposition column for exactly this). And the ledger's named D4 trap:
**the §22.2 "license/security review" CI gate is only the automated scan;
the legal D4 determination is P10.19's — this record must not be recorded
as D4 closed** (§6).

## 2. The gate checklist — the §22.2 gate list mapped to the actual ci.yml jobs

The job list below is enumerated from `.github/workflows/ci.yml` AS IT
STANDS on 2026-11-14 (23 jobs). Every job is WRITTEN-UNRUN — no CI
execution has ever happened for any of them (each was written in a
no-execution loop, and the first human CI run is the recorded verification
point). The **Result**, **Log/output location**, and **Disposition** cells
are the run record; they are EMPTY until the human run fills them.

| # | §22.2 gate | ci.yml job (as written) | Result | Log/output location | Disposition (for any failed/skipped gate) |
|---|---|---|---|---|---|
| G1 | `cargo fmt --check` | `static-checks` (step "Format check": `cargo fmt --all -- --check`) | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G2 | `cargo clippy --workspace --all-targets -- -D warnings` (allowlist only by explicit review — there is NO clippy allowlist in the workflow today; adding one is a recorded review event, never a silent edit) | `static-checks` (step "Clippy (deny warnings)") | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G3 | Dependency-rule check (the architecture gate; AGENTS.md §8) | `static-checks` (step "Dependency-rule check (P1.04)": `python tools/check_dependency_rule.py`) | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G4 | Buf lint (`proto/`) | `static-checks` (step "Buf lint (P1.18)") | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G5 | `cargo test --workspace` (unit leg) | `unit-tests` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G6 | `cargo test --workspace` (integration leg; PostgreSQL 18.6 + NATS 2.15.2 services) | `integration-tests` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G7 | Migration tests and schema compatibility (up/down of ALL migrations; the P10.13 suite is the release-hardening extension) | `migration-tests` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G8 | Scenario regression suite — exit-gate scenario run + replay verdict assertion + P4.31 fault-injection and shadow suites | `scenario-regression` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G9 | TelemetrySource provider contract suite | `telemetry-contract` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G10 | Phase 3 deterministic scenario suite + benchmark-record STRUCTURE assertions (never a runtime threshold) | `phase3-scenario-regression` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G11 | Edge adapter conformance suite (`flexcore-edge-adapter-conformance-suite-v1`) + edge security suite + load harness (structural) | `edge-agent-suite` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G12 | Secret scan (gitleaks) — container/dependency scanning half 1 | `security-and-supply-chain` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G13 | Release-profile auth-stub gate (dev-auth must never be default) | `security-and-supply-chain` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G14 | License scan (cargo-deny `licenses`) — the §22.2 "license/security review" AUTOMATED HALF ONLY (see §6) | `security-and-supply-chain` (step has `continue-on-error: true`, written before deny.toml existed — an honest disposition is REQUIRED for this step either way) and `edge-license-gate` (the FAILING gate backed by deny.toml) | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G15 | SBOM generation (CycloneDX `cargo cyclonedx --all`) | `security-and-supply-chain` (artifact `sbom`) | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G16 | Buf breaking-change detection (`proto/`) — NOTE: this job is `pull_request`-conditional (`if: github.event_name == 'pull_request'`); on a release run it MUST be exercised explicitly (e.g. against the release base), and its result recorded — a trigger condition is not a pass | `buf-breaking` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G17 | Frontend lint/type-check/test/build (console; `npm run lint` / `typecheck` / `test` / `build`) | `console-quality` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G18 | Advisory scan (cargo-audit) — dependency scanning | `edge-supply-chain` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G19 | SBOM + container scan (Syft) | `edge-supply-chain` (artifact `edge-supply-chain-sbom`) | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G20 | Plugin SDK conformance suite (`flexcore-plugin-sdk-conformance-suite-v1`) + untrusted-plugin negatives + structural perf harness + no-dylib test twin | `plugin-sdk-conformance` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G21 | SDK scenario-regression suite (`flexcore-plugin-sdk-scenario-regression-v1`) | `sdk-scenario-regression` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G22 | Plugin manifest validation (the P7.05 validator IS the gate) | `plugin-manifest-validation` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G23 | WIT compatibility validation (`wasm-tools component wit wit/` at the recorded pin) + the P7.40 pin-record consistency check | `wit-validate` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G24 | Buf lint + breaking for the Class C worker contract (`python/workers/contracts`) — the breaking step is again `pull_request`-conditional; exercise explicitly on a release run | `worker-contract-buf` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G25 | No-dylib lexical gate (ADR-006) | `no-dylib-gate` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G26 | Python Ruff + type checks + pytest (research workers + worker template/tests) — FAIL-CLOSED BY RECORDED STATE: the pin gate step FAILS while the research-stack pins are UNPINNED-REVERIFY/DECISION-POINT (no version is invented by CI). On a release run this job either (a) runs green after the human lands verified pins, or (b) is recorded FAILED/SKIPPED with a disposition approved for the release — never silently dropped | `research-python-gates` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G27 | Research Rust harness gates (`cargo test -p flexcore-research` + frozen benchmark-artifact STRUCTURE check) | `research-rust-harness-gates` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G28 | Research-worker conformance suite (Python leg) — same fail-closed pin gate as G26 | `research-worker-conformance` | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |
| G29 | Research reproducibility check — FAIL-CLOSED BY RECORDED STATE: the readiness gate exits 1 until the frozen P8.06 results artifact, the P8.38 tolerance pre-registration (X-8), and a P8.20 reproducibility record exist. A green run is impossible today by design; the disposition must record exactly that | `research-reproducibility-check` | `<UNRUN — expected first-run outcome: FAIL by the recorded readiness gate>` | `<EMPTY>` | `<EMPTY>` |
| G30 | Repository verification commands (AGENTS.md §6): `python tools/verify_docs.py`, `python tools/validate_agent.py` | human runbook step (§3 block, step 8; also packaged in the §7 job) | `<UNRUN>` | `<EMPTY>` | `<EMPTY>` |

Honest expectations the record carries UP FRONT (from the recorded state
of the repository, not from any execution):

- **G26/G28 will fail** on the first run by the recorded fail-closed pin
  state (the CI file's own error text names the re-verify procedure) — a
  disposition approved for the release is required, or the human lands
  verified pins first.
- **G29 will fail** on the first run by its recorded readiness gate — the
  comparison artifacts do not exist.
- **G14's `security-and-supply-chain` cargo-deny step** carries
  `continue-on-error: true` (a pre-deny.toml artifact) — the FAILING gate
  is `edge-license-gate`; the disposition must treat the two distinctly.
- **G2 has no allowlist mechanism in the workflow** — if a first run
  surfaces clippy findings, the fix is code or an explicitly reviewed
  allowlist, never a silent `-A` flag.
- The F-P10.07-5 clock-median finding
  (`ledger/phase-10-v1-release-hardening/07_edge_offline_tests.md` §6)
  predicts a failing workspace test (`crates/edge-agent/src/clock_quality.rs`
  boundary unit test) on the first `cargo test --workspace` — a candidate
  gate outcome to be recorded, never pre-judged as passed or fixed here.
- No gate is waived by this record. Every failed/skipped gate needs a
  recorded disposition approved for the release (the ledger acceptance
  (a)); zero dispositions exist because zero runs exist.

## 3. The release-candidate build procedure — exact commands, NEVER RUN

The whole procedure below is the human's. Nothing in this loop executed
any of it. Commands are run from a clean checkout of the gated commit
(the ledger's verification method: re-run from a clean checkout of the
candidate commit and compare).

```sh
# 0. The gated commit — the run and the build happen on the SAME commit:
git rev-parse HEAD
git status --porcelain            # must be empty — a dirty tree is not a candidate
git log -1 --format='%H %ci %s'   # recorded next to the run report

# 1. Rust: format/clippy/tests (the G1–G6 core; full job list in §2):
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
#    with services for the integration leg (PostgreSQL 18.6, NATS 2.15.2)
#    and FLEXCORE_TEST_DATABASE_URL / FLEXCORE_TEST_NATS_URL set as the
#    workflow env block records them.

# 2. Migration tests (G7):
cargo test -p flexcore-postgres-adapter

# 3. Python suites (G26/G28) — ONLY after the recorded pin re-verify has
#    landed real pins (the fail-closed gate names the procedure):
#      ruff check apps/worker/python python/workers
#      mypy <the module list recorded in the research-python-gates job>
#      python -m pytest apps/worker/python/{reproducibility,gnn_worker,marl_worker,andes_worker,portfolio_analytics,drill}/tests python/workers/tests -v

# 4. Proto/WIT (G4/G16/G23/G24):
#      buf lint proto            && buf breaking proto --against <base>
#      buf lint python/workers/contracts && buf breaking python/workers/contracts --against <base>
#      wasm-tools component wit wit/     # at the recorded 1.239.0 pin (ASSUMPTION — re-check the release page first)

# 5. Console (G17):
cd console
npm install        # no committed lockfile yet — switch to npm ci once one lands (the recorded P5.28 decision)
npm run lint && npm run typecheck && npm run test && npm run build
cd ..

# 6. Supply chain (G12/G13/G14/G15/G18/G19): gitleaks; cargo deny check
#    licenses; cargo audit; cargo cyclonedx --all --format json; Syft.
#    These produce the SBOM/scan artifacts P10.19/P10.20 consume — they do
#    NOT close D4 (§6).

# 7. THE RELEASE-CANDIDATE BUILD (the ledger step 3):
cargo build --release --workspace

# 8. Repository verification (AGENTS.md §6; the ledger P10.18 step 4):
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py

# 9. Artifact hashes (feeds §4):
sha256sum target/release/<each released binary> > release-candidate-hashes.txt
```

Every command above: **NOT RUN in this loop — banned** (no build, no test,
no install, no CI). The §5 record is where their actual results go.

## 4. The candidate-identity procedure — what fills the `<PENDING P10.18>` freeze cells

| Identity field | How it is produced (human step) | Value |
|---|---|---|
| Commit | `git rev-parse HEAD` of the gated commit — the SAME commit every §2 gate ran on (the ledger's named risk: a gate passing on a different commit than the candidate is a finding, not a pass) | `<EMPTY — filled by the human run>` |
| Tag | A release-candidate tag (proposed form `v1.0.0-rc<N>`; the actual name and creation are the human's — tagging/publication itself is P10.28, prepared-NOT-EXECUTED) | `<EMPTY — no tag created in this loop>` |
| Build ID | The CI run ID of the §7 `release-candidate-build` dispatch (or the human's recorded local build id) | `<EMPTY>` |
| Artifact hashes | `sha256sum` over the `cargo build --release --workspace` binaries (§3 step 9) | `<EMPTY>` |

These four values are exactly what fills the `<PENDING P10.18>` cells of
`ledger/phase-10-v1-release-hardening/01_release_scope_freeze.md` §6.1.
Until they are filled, that freeze is provisional by its own §6.1 rule and
no release-notes content (P10.26) may be derived from it. If the candidate
changes after the identity is recorded, the P10.01 §6.2 drift procedure
applies: the affected verifications (P10.03–P10.17) re-run against the new
identifier — this record re-runs with them, never inherits.

## 5. The run record — EMPTY template (CI green is a go/no-go INPUT to P10.27, never a claim)

| Field | Value |
|---|---|
| Run date | `<EMPTY>` |
| Executor | `<EMPTY>` (human) |
| Candidate commit | `<EMPTY>` |
| Candidate tag / build ID | `<EMPTY>` / `<EMPTY>` |
| Clean-checkout confirmation | `<EMPTY>` |
| Per-gate results (G1–G30 of §2) | `<EMPTY>` — one row per gate, each with result + log/output location + disposition for any failed/skipped gate |
| Skipped gates (with reasons) | `<EMPTY>` — a skipped gate is not a passed gate |
| Failed gates (with dispositions approved for the release) | `<EMPTY>` |
| Build outcome (`cargo build --release --workspace`) | `<EMPTY>` |
| Artifact hashes | `<EMPTY>` |
| Repository verification output (verify_docs / validate_agent / check_dependency_rule) | `<EMPTY>` |
| Comparison to the filed report (the ledger verification method: re-run from a clean checkout) | `<EMPTY>` |

No cell above is pre-filled with a success. There are zero observed gate
results and zero claimed green runs; the workspace has NEVER had a CI
execution of any of these jobs.

## 6. Explicit non-claims (binding on this record)

1. **The §22.2 "license/security review" CI gate is only the automated
   scan.** The legal D4 determination is P10.19's; P10.18 must not be
   recorded as D4 closed (the ledger's own named trap, carried verbatim in
   substance).
2. **No green CI run is claimed.** All 23 jobs are WRITTEN-UNRUN.
3. **No "zero failures" is claimed from suites that have never run** —
   including the P10.03–P10.08 test suites, which run as workspace tests
   inside G5/G6; their content is written, their execution evidence does
   not exist.
4. **No SBOM, advisory result, or scan output exists** — P10.19/P10.20
   consume the §3 step-6 artifacts when the human run produces them.
5. **The candidate does not exist.** No artifact hash, build ID, or tag is
   recorded anywhere in this loop.

## 7. The additive CI job — `release-candidate-build` (workflow_dispatch only)

`.github/workflows/ci.yml` gained ONE additive job and ONE additive
trigger line (`workflow_dispatch:`), both recorded here:

- The job `release-candidate-build` runs **only on explicit
  `workflow_dispatch`** (`if: github.event_name == 'workflow_dispatch'`);
  it never runs on push or pull_request, so no existing job's behaviour,
  trigger, or semantics changed.
- It packages §3 steps 0/1(core)/7/8/9: clean-tree check, pinned toolchain,
  `cargo build --release --workspace`, the repository verification
  commands, and the sha256 artifact-hash capture uploaded as the
  `release-candidate-identity` artifact — the inputs of §4.
- **The job itself is WRITTEN-UNRUN** — its dispatch IS the human gate.
  Its first execution is the verification point, exactly like every job
  added in Phases 5–8.

## 8. Findings recorded by this record

| # | Finding | Disposition |
|---|---|---|
| F-1 | Zero of the 23 ci.yml jobs has ever executed; the first human CI run is the verification point for the entire gate set | Binding honesty statement |
| F-2 | G26/G28/G29 are fail-closed by recorded state (pin gaps; missing reproducibility artifacts) — their first-run failures are EXPECTED and require recorded dispositions, never silent skips | §2 notes; carried to the human run |
| F-3 | Two breaking-change gates (G16, G24's breaking step) are pull_request-conditional; a release run must exercise them explicitly | §2 notes |
| F-4 | The F-P10.07-5 clock-median defect predicts a first-run workspace-test failure (cited, not fixed — `crates/**` is outside this subtask's ownership) | Carried; recorded in `17_operational_risk_review.md` §3 R-7 as well |
| F-5 | No clippy allowlist exists in the workflow; adding one is an explicit reviewed event | §2 G2 note |
| F-6 | The candidate identifier is EMPTY; P10.01 §6.1 stays `<PENDING P10.18>` until the human run fills it | §4 procedure |

## 9. Reviewer-verification procedure (human)

1. Confirm §1/§5/§6: no gate result, no build artifact, no hash, no tag,
   no green run is claimed anywhere; every Result cell is `<UNRUN>`.
2. Enumerate `grep -nE '^  [a-z0-9_-]+:' .github/workflows/ci.yml` and
   confirm the §2 job list matches the file's actual jobs, that
   `release-candidate-build` is dispatch-only, and that no pre-existing
   job block was modified (a diff against the pre-P10.18 file shows only
   the `workflow_dispatch:` trigger line and the appended job).
3. Confirm §3's commands match the workflow's recorded steps (toolchain
   install pattern, service images, console script names, wasm-tools pin)
   and that none was executed here.
4. Confirm §6's non-claims, especially the D4 sentence.
5. Confirm the ledger tick reads "implemented, awaiting human
   build/test/execution".

## 10. Filling commands (human, at execution time)

```sh
# The run record (§5) is filled from the actual run:
git rev-parse HEAD && git status --porcelain
cargo build --release --workspace
python tools/verify_docs.py
python tools/validate_agent.py
python -I tools/check_dependency_rule.py
sha256sum target/release/<binaries>

# Or: dispatch the release-candidate-build job and read its
# release-candidate-identity artifact; then fill P10.01 §6.1:
#   commit / tag / build ID / artifact hashes  <- the four <PENDING P10.18> cells

# Registry verification after any edit to this record:
python tools/verify_docs.py
python tools/validate_agent.py
```

## 11. Status

**WRITTEN-UNRUN.** The CI gate run and the release-candidate build are the
human gates; neither has happened; the candidate artifact does not exist;
the run record is an EMPTY template. CI green, when it eventually happens,
is an INPUT to the P10.27 go/no-go record — never a release decision made
by this file. The P10.18 ledger tick reads "implemented, awaiting human
build/test/execution".
