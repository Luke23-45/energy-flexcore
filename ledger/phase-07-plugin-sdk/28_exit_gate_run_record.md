# P7.28 — Exit-gate run (swap + untrusted-plugin proofs, witnessed): run book + EMPTY record, status PREPARED/UNRUN

Date: 2026-11-13. Subtask: P7.28, Phase 7 ledger (`ledger_phase_7.md`
§P7.28). Owner roles: architect + security reviewer (per the ledger).

**Run-status honesty (AGENTS.md rule 7; the loop bans in
`ledger/phase-07-plugin-sdk/ENTRY_NOTE.md`):** the exit-gate run has **NOT
happened** and cannot happen in this loop — nothing was built, executed,
captured, or witnessed. This record is the complete witnessed-run book plus
the record template with EMPTY result cells, in the same PREPARED/UNRUN
pattern as `ledger/phase-06-edge-protocol/24_exit_gate_lab_run_record.md`.
**Every measured, captured, or witnessed cell below is deliberately EMPTY.
No figure in this record is measured. The human executing the run fills these
cells; nothing here stands in for that evidence.**

## 0. Gate status: NOT HELD

| Field | Value |
|---|---|
| §23 Stage 7 exit gate (verbatim substance, `CHECKLIST.md`) | "Swap one optimizer/grid provider without changing application/domain code; untrusted plugin cannot access ungranted capabilities." |
| Gate status | **NOT HELD.** The gate is NOT HELD until a human executes this run book, the evidence pack (§3–§5) is filled from actual output, and the independent witness reproduces it from the pack alone. No checkbox in `CHECKLIST.md` is ticked by this record. |
| What exists today | The suite files and their decision records (P7.21/P7.22/P7.23, P7.12/P7.13, P7.36, P7.26, P7.35), all WRITTEN-UNRUN; the pin record (`40_toolchain_pinning.md`); the profiles and manifests (P7.15/P7.05); the CLI job wiring (P7.27), UNRUN. |
| Automated equivalents | EXIST AS WRITTEN TESTS ONLY, all **UNRUN** (§5). None of them substitutes for this run: the run book adds the real-provider composition, the real diff check, and the witness — none of which any in-repo test can perform. |
| Blocking items | The first human build/test pass of the workspace (P7.01 G-1, UNRUN); the bindings materialisation (P7.13 §7 human gate); the seam-gap exposure of §1 P-3 (sim-runner path deps + integration file, NOT APPLIED); no plugin artifact exists (no signed component has ever been built). |

## 1. Prerequisites (all must hold before the run starts)

| # | Prerequisite | Source | Status at writing |
|---|---|---|---|
| P-1 | Workspace builds and the existing suites pass per the pin record's procedure (`cargo test --workspace` minimum; `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings` per the P7.27 gate matrix) | `40_toolchain_pinning.md` §5; `26_27_35_ci_performance.md` §2 | UNRUN |
| P-2 | Wasmtime pin applied and recorded: `=50.0.1` (floor ≥ 50.0.0; carries CVE-2026-115889, CVE-2026-115893, CVE-2026-115384); the three GHSA advisory pages re-checked with the check date recorded (P7.13 §9 human gate) | `40_toolchain_pinning.md` §2–§3; `12_13_enforcement_negatives.md` §9 | PIN RECORDED; advisory re-check UNRUN |
| P-3 | **The P7.22 seam-gap exposure applied by the coordinator/human (exactly these two edits + one file — nothing else):** (a) add the path dependency `flexcore-kernel = { path = "../../crates/flexcore-kernel" }` to `apps/sim-runner/Cargo.toml`; (b) add the path dependency `flexcore-optimization-api = { path = "../../crates/flexcore-optimization-api" }` to the same manifest (both edges are allowed composition-root edges per `tools/check_dependency_rule.py` — re-run the tool after the edit); (c) create `apps/sim-runner/tests/swap_p7_22_integration.rs` per the recorded shape in `21_22_23_conformance_swap_untrusted.md` §3.3 gap 2: register the REAL providers (`HighsTier1Provider::new(Arc<dyn Tier1Backend>)`, `ClarabelTier2Provider::new(Tier2ProviderConfig)`, `PandapowerGridProvider::new(backend, scope)`) from a real plan through `FnFactory`, swap via `swap_provider`, and drive `run_validation_gate`/`run_feasibility_gate` (`flexcore_application::command_gates`) over the swapped output — the ADR-009 independent-validation assertion against the REAL application seam | `21_22_23_conformance_swap_untrusted.md` §3.3 gaps 1–3 | NOT APPLIED (deliberately recorded, not performed — the P7.22 wave owned no `Cargo.toml`) |
| P-4 | The example component artifact materialised behind the `generated-bindings` feature (the human's wit-bindgen/`cargo component` run) — needed only for the §23 Stage 7 "runs an example component" deliverable row; the swap and untrusted proofs do NOT require it (they are host-API and kernel-seam proofs) | `40_toolchain_pinning.md` §5; `12_13_enforcement_negatives.md` §7 | NOT MATERIALISED (the sandbox/demo row is recorded UNRUN if absent — never faked) |
| P-5 | Worker template runnable for the drill's Class C faults: the `==UNPINNED-REVERIFY` requirements pins replaced with verified versions per the recorded re-verify procedure; `buf lint`/`buf breaking` first verdicts | `17_18_workers_supervision.md` §6; `26_27_35_ci_performance.md` §2.1 | UNPINNED-REVERIFY |
| P-6 | Versions recorded BEFORE any fault (§3.1): git SHA, Rust toolchain, wasmtime, wasm-tools, WIT world pins, worker template, plugin/manifest versions, profile versions | this record §2 step 0; `40_toolchain_pinning.md` §6 | EMPTY cell (§3.1) |
| P-7 | Independent witness named (the ledger verification method: the witness re-runs the run book from the evidence pack alone, on a clean build on a clean machine) | ledger P7.28 step 3 | EMPTY cell (§3.6) |

## 2. The run book (the executable procedure)

Steps are executed in order; a deviation at any step is recorded in §3.6,
never absorbed silently.

| Step | Action | What is recorded |
|---|---|---|
| 0 | Record the environment: apply P-3 exactly as written; re-run `python -I tools/check_dependency_rule.py` (must stay OK; the two path deps are composition-root edges); record every version per P-6 | §3.1 table, filled; the dependency-tool output line |
| 1 | Run the swap proof (kernel level): `cargo test -p flexcore-kernel --test swap_p7_22 -- --nocapture` | Per-swap outcomes (§3.2 rows 1–3): the HiGHS-class → Clarabel-class optimizer swap and the pandapower-class → OpenDSS-class grid swap; the blocked-then-allowed refusals; the audit ordering (refused < Ready→Draining < Draining→Stopped < VERSION_CHANGE) |
| 2 | Run the swap proof (real-provider level): `cargo test -p flexcore-sim-runner --test swap_p7_22_integration -- --nocapture` | §3.2 rows 4–6: the real providers register/construct/swap; the ADR-009 independent validation verdicts before AND after each swap; the pre-declared equivalence criteria (feasibility identical; objective within tolerance 0 — the integral-data fixture basis; §9.2 provenance + diagnostics non-empty) |
| 3 | The zero-diff check: `git diff --stat` over `crates/flexcore-application/`, `crates/flexcore-domain/`, `crates/flexcore-ports/` taken between the pre-swap and post-swap states (both swaps) | §3.2 row 7: the diff MUST be empty; any diff is a gate FAILURE, not a note |
| 4 | Run the untrusted-plugin proof: `cargo test -p flexcore-plugin-host --test untrusted_plugin_p7_23 -- --nocapture`; then the enforcement negatives `--test enforcement_p7_12` and the sandbox negatives `--test sandbox_p7_13`; then the P7.29 attack layer (`--test security_p7_29` and, kernel-side, `cargo test -p flexcore-kernel --test security_p7_29_kernel`) | §3.3: every layered refusal (L1–L5) observed; the never-granted walks; the cross-tenant attempts; the ADR-010 attempts; the P7.29 attack chains; every denial audited (the audit-coverage note of `21_22_23...` §6 DEFAULT 7 — L3 direct-grant refusals return without a broker audit record — is honoured when reading the results) |
| 5 | Run the conformance suite: `cargo test -p flexcore-kernel --test conformance_p7_21 -- --nocapture` | §3.4: the run record line per class (`class=… suite=flexcore-plugin-sdk-conformance-suite-v1 version=1 cases=n/5`); the suite identity + claim boundary pinned by the suite's own test |
| 6 | The drill (P7.30-adjacent): the §19.2(6) fault set over plugin host, workers, and registry per `ledger_phase_7.md` §P7.30 — its own subtask and its own record; this run book carries the per-fault table skeleton ONLY so the exit-gate pack is complete | §3.5: per-fault detection/behaviour/recovery/loss/command-integrity — ALL EMPTY here; the drill record owner fills it (P7.30). This run book does NOT claim the drill |
| 7 | Collect the evidence pack: the §3 tables filled from actual output + the CI job logs (the `plugin-sdk-conformance` job, if CI is green by then) + the P7.29 security report + this record | Pack path: this file + `29_security_test_report.md` + the drill record |
| 8 | The independent witness (named in §3.6) re-runs the run book FROM THE EVIDENCE PACK ALONE on a clean build on a clean machine and confirms reproduction; additionally, the witness re-runs at least TWO named cases of the untrusted/conformance suites and reproduces their recorded outcomes (the P6.28 two-case witness discipline, applied here) | §3.6: witness name, date, reproduction outcome, the two spot-check cases and their outcomes |
| 9 | Record the gate decision (pass/fail per `CHECKLIST.md` item) with the witness's name and date; a failed item is recorded as failed — never absorbed | §4 mapping, filled; `CHECKLIST.md` completed by the coordinator at phase close |

## 3. Results tables — ALL CELLS EMPTY (fill at the witnessed run)

### 3.1 Versions and environment

| Item | Value (EMPTY — fill at the run) |
|---|---|
| FlexCore git SHA | |
| Rust toolchain (expected pin: 1.99.1, `rust-toolchain.toml`) | |
| Wasmtime (expected pin: `=50.0.1`; the version actually compiled) | |
| GHSA advisory re-check date (P7.13 §9 gate) | |
| `wasm-tools` (ASSUMPTION 1.239.0 — re-check the release page; record the ACTUAL version used) | |
| `wit-bindgen` / `cargo component` (UNPINNED-REVERIFY — record the ACTUAL versions used) | |
| WIT world pins (`flexcore:optimizer@1.0.0`, `flexcore:grid@1.0.0`, `flexcore:telemetry@1.0.0`) | |
| Worker template / `flexcore.worker.v1` contract version | |
| Plugin manifest + provenance versions exercised | |
| Profile versions (`flexcore-profile-v1` documents, version 0.5.0) | |
| Host OS + hardware (run machine, and the witness's clean machine) | |
| `check_dependency_rule.py` output after P-3 | |

### 3.2 Swap proof (§23 Stage 7 exit gate, first half)

| # | Proof element | Result (EMPTY) |
|---|---|---|
| 1 | Optimizer swap (HiGHS-class → Clarabel-class) through `swap_provider` + drain-before-replace only | |
| 2 | Blocked-then-allowed: `LIFECYCLE_REFUSED`/`REPLACEMENT_BLOCKED_UNDRAINED` while Active and mid-drain; allowed only from `Stopped` | |
| 3 | Grid-provider swap (pandapower-class → OpenDSS-class stand-in); declared analysis class unchanged across the swap (D3 fidelity point) | |
| 4 | Real providers registered/constructed/swapped (`swap_p7_22_integration`) | |
| 5 | ADR-009 independent feasibility validation verdicts before AND after each swap (the optimizer's own word is never the verdict) | |
| 6 | Pre-declared equivalence: same feasibility verdict; objective within tolerance 0; §9.2 provenance + diagnostics non-empty | |
| 7 | `git diff` over application/domain code between pre-swap and post-swap: **MUST be empty** (a non-empty diff fails the gate) | |
| 8 | Audit ordering: refused < Ready→Draining < Draining→Stopped < VERSION_CHANGE; version record carries OLD version with NEW in reason | |
| 9 | Solver-class declarations correct for each provider (HiGHS = LP/QP/MIP not SOCP; Clarabel = convex conic not MISOCP; the MILP/Clarabel mismatch impossible) | |

### 3.3 Untrusted-plugin proof (§23 Stage 7 exit gate, second half)

| # | Proof element | Result (EMPTY) |
|---|---|---|
| 1 | Layered walk L1–L4 over the never-granted set (`dispatch.authorize`, `dispatch.issue`, `secret.read`, `network.any`): earliest-layer refusal + next-layer-still-refuses | |
| 2 | Cross-tenant telemetry attempts (other tenant / other site / tenant-wide ask): `SCOPE_OUTSIDE_GRANT` ×3, each audited | |
| 3 | ADR-010 attempts (authorize, issue, verdict-shaped ids, validate-request misfence): refused at every layer; the forward returns `()` | |
| 4 | Enforcement negatives (P7.12): over-broad, unknown, duplicate, drift, frozen set, audit-or-refuse | |
| 5 | Sandbox negatives (P7.13): ambient access, exhaustion teardown contracts, grant forgery, bound-lie | |
| 6 | P7.29 attack chains: supply-chain (tamper/downgrade/unprovenanced), escalation (expansion/restart/drain-deadline/forged profile), containment, ordered audit trail | |
| 7 | Conformance suite (§3.4) and the per-class records: same case set for classes A, B, C | |
| 8 | Every denial audited (honouring the recorded L3 audit-coverage note) | |

### 3.4 Conformance suite run record

| Item | Value (EMPTY) |
|---|---|
| Suite id | `flexcore-plugin-sdk-conformance-suite-v1` (fixed) |
| Suite version | `1` (fixed) |
| Run command | `cargo test -p flexcore-kernel --test conformance_p7_21 -- --nocapture` |
| Run ID / CI run URL | |
| Per-class record lines (A / B / C) | |
| Class C proto mirror rules observed (`contract_version`, `request_id`, `seed`, `seed_supported`) | |
| Claim boundary restated by the suite's own pin test | |

### 3.5 Failure-mode drill (P7.30-adjacent — its OWN record owns the results)

The drill is ledger subtask P7.30 over the §19.2(6) fault set (plugin-host
crash, worker crash, worker timeout, registry unavailable, plugin crash
mid-schedule, malformed plugin result, oversized plugin result, profile with
a missing plugin). This run book carries the skeleton so the exit-gate pack
is complete; **the results belong to the P7.30 drill record and this record
claims nothing.**

| Fault | Detection (EMPTY) | Behaviour (EMPTY) | Recovery time (EMPTY) | Schedule issued/duplicated/lost (EMPTY) |
|---|---|---|---|---|
| Plugin-host crash | | | | |
| Worker crash | | | | |
| Worker timeout | | | | |
| Registry unavailable | | | | |
| Plugin crash mid-schedule | | | | |
| Malformed plugin result | | | | |
| Oversized plugin result | | | | |
| Profile with a missing plugin | | | | |

### 3.6 Witness record

| Item | Value (EMPTY) |
|---|---|
| Executed by / date | |
| Witness name (independent; did not build the SDK) / date | |
| Witness's clean machine + clean-build reference | |
| Witness reproduction outcome (full run book) | |
| The TWO spot-check cases the witness re-ran + their outcomes | |
| Exit status of run-book steps 0–9 | |
| Deviations from this record | |
| Gate decision (pass/fail per CHECKLIST.md item) + signature | |

## 4. Exit-check mapping (§23 Stage 7 and `CHECKLIST.md`)

| `CHECKLIST.md` item | Evidence this run must produce | Status |
|---|---|---|
| Optimizer provider swapped without changing application/domain code | §3.2 rows 1, 4–7, 9 | EMPTY — UNRUN |
| Grid provider swapped without changing application/domain code | §3.2 rows 3–7 (fidelity point recorded) | EMPTY — UNRUN |
| Untrusted plugin cannot access ungranted capabilities (WASM class) | §3.3 rows 1–6 + `29_security_test_report.md` §2/§3 | EMPTY — UNRUN |
| Untrusted plugin cannot access ungranted capabilities (gRPC class) | The P7.17/P7.18 worker seam + the drill's Class C faults (§3.5) — the worker-template negatives remain the recorded seam; never claimed from the host suite alone | EMPTY — UNRUN |
| No arbitrary Rust dylib loading | P7.35 CI job + scratch-branch demonstration (separate evidence; cited, not re-run here) | UNRUN |
| Protected kernel intact | P7.12 enforcement matrix + the protected-contract refusals in §3.3 | EMPTY — UNRUN |
| Sandbox negative tests pass | §3.3 row 5 (the behavioural, real-component half stays the P7.13 §7 human gate) | EMPTY — UNRUN |
| Signing/provenance/SBOM in place | P7.14 suite + the P7.29 supply-chain chains (§3.3 row 6); SBOM generation is P7.27 CI | EMPTY — UNRUN |
| WASI 0.2 baseline recorded; WASI 0.3 not claimed | P7.09 record (cited); no WASI 0.3 claim exists anywhere | RECORDED (citation) |
| CI gates green on the SDK | The P7.27 jobs' run logs | EMPTY — UNRUN |
| Security tests pass with zero open high/critical findings | `29_security_test_report.md` §3 filled from actual output + the independent re-run | EMPTY — NOT CLAIMABLE YET |
| Operator console shows plugins, permissions, versions, health | P7.25 walkthrough record (the other wave's deliverable) | per P7.25's own record |
| SDK documentation and examples complete | P7.24 record | per P7.24's own record |
| Handover to Phase 8 written | P7.31 record | PENDING (its own subtask) |
| Retrospective and v0.5.0 release-gate record written | The P7.32 record (`30_retrospective_release_gate.md`) | PENDING (coordinator closes) |

## 5. Automated equivalents — WRITTEN TESTS, status UNRUN

| Automated equivalent | What it covers in-process | What it does NOT cover | Status |
|---|---|---|---|
| `crates/flexcore-kernel/tests/swap_p7_22.rs` | The seam property over local doubles: both swaps through `swap_provider` + drain-before-replace, blocked-then-allowed, audit ordering, pre-declared equivalence criteria, class discipline | The REAL providers, the REAL independent-validation seam, the real `git diff` | WRITTEN, **UNRUN** |
| `apps/sim-runner/tests/swap_p7_22_integration.rs` (to be created per P-3) | The real-provider swap + `run_validation_gate`/`run_feasibility_gate` over the swapped output | The witnessed reproduction | NOT CREATED (seam gap 2) |
| `apps/plugin-host/tests/untrusted_plugin_p7_23.rs` | The layered end-to-end negatives (L1–L5) | A live WASM component executing the attacks (L5 backstop only) | WRITTEN, **UNRUN** |
| `apps/plugin-host/tests/{enforcement_p7_12,sandbox_p7_13}.rs` | The door, broker, and sandbox unit negatives | Real-component behaviour (the P7.13 §7 list) | WRITTEN, **UNRUN** |
| `crates/flexcore-kernel/tests/conformance_p7_21.rs` | The 44-case class-agnostic suite incl. the per-class records and the claim-boundary pin | CI execution (P7.27 job) | WRITTEN, **UNRUN** |
| `apps/plugin-host/tests/security_p7_29.rs` + `crates/flexcore-kernel/tests/security_p7_29_kernel.rs` | The P7.29 attack chains (supply chain, escalation, containment, audit trail) | The findings clearance (human gate) | WRITTEN, **UNRUN** |
| `apps/plugin-host/tests/perf_harness_p7_26.rs` | The four host-side overheads (structural assertions; measured cells are P7.26's) | Target-hardware measurements | WRITTEN, **UNRUN** |

## 6. Honest limitations

1. **The gate is NOT HELD.** Nothing in this record executes the run, and no
   cell above is filled. The §23 Stage 7 exit gate is met only when a human
   executes §2, §3 is filled from actual output, and the independent witness
   reproduces the run from the pack alone (ledger P7.28 step 3).
2. **The seam-gap exposure (P-3) is recorded, not applied.** The two sim-runner
   path dependencies and the integration test file are the coordinator/human's
   to apply at the witnessed run; applying them changes workspace dependency
   edges and must be re-verified with `check_dependency_rule.py` (both edges
   are composition-root-allowed per the P7.22 record's analysis).
3. **No signed component artifact exists.** The swap and untrusted proofs run
   against host APIs and kernel seams; the "runs an example component" row of
   the Stage 7 deliverables needs the P-4 materialisation, and the
   real-component hostile run stays the P7.13 §7 human-gate list. Absence is
   recorded, never faked.
4. **The gRPC-class untrusted-plugin proof is partial by construction.** The
   Class C enforcement cells are the P7.17/P7.20 seams (recorded, not built);
   the drill's worker faults need the P-5 pins. The CHECKLIST row for the
   gRPC class can only cite the worker-supervision suite and the drill — this
   run book does not stretch the host suite over it.
5. This record adds no code and touches no source file; it owns exactly one
   new ledger file plus the P7.28 ledger tick.
