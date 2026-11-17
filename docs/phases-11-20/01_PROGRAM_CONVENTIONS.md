# Program v2 Conventions — Rules Every Phase 11–20 Ledger Inherits

Date: 2026-11-14. This file is binding for all work under `docs/phases-11-20/`.
It carries forward the Phases 0–10 working discipline unchanged and adds the
few things a second program needs. Where a rule here conflicts with a rule in
`AGENTS.md`, the stricter one wins.

## 1. The loop bans (carried verbatim from Phases 0–10)

1. **Do not install anything.** No `cargo add`/`cargo install`, no `pip
   install`, no `npm install`, no tool downloads, no docker pulls.
2. **Do not build.** No `cargo build/check/run`, no `tsc`, no `vite`, no
   `buf`, no `wasm-tools`, no docker image builds.
3. **Do not run tests.** No `cargo test`, no `clippy`, no `fmt`, no `pytest`,
   no `vitest`, no CI runs. **Write complete test files** where a subtask
   calls for tests; their execution is a human gate.
4. No `todo!()` / `unimplemented!()` / `TODO` / `FIXME` placeholders, ever.
5. Permitted verification tools only: `python tools/verify_docs.py`,
   `python tools/validate_agent.py`, `python -I
   tools/check_dependency_rule.py`. Python heredocs (`python -I`) are
   permitted **solely** for exact-match ledger checkbox ticks with
   count==1 assertions, as in Phases 0–10.

## 2. Ledger discipline

1. Subtask IDs are `P<NN>.<xx>` continuing the 0–10 numbering (P11.01 ...).
2. A checkbox is ticked **only** as "implemented, awaiting human build/test"
   with a decision-record citation: `- [x] ... (decision record:
   \`records/NN_slug.md\`)`. Never as "tested", "verified", "passed", or
   "works".
3. Tick flips are applied by `python -I` heredoc with exact-match
   count==1 assertions, or by the Write/Edit tools for whole files.
4. Every batch of implemented subtasks gets a decision record under the
   phase's `records/` directory (created at implementation time, not during
   planning), following the 0–10 record conventions.
5. Phase close (when a phase's implementation completes): a HANDOVER, a
   CHECKLIST, and a close/exit-gate record are added to the phase directory,
   mirroring the Phase 0–10 structure.

## 3. Planning-phase ledger state

At planning time (now), every ledger line is `- [ ]` and the ledger header
reads: status NOT STARTED; gate = human review of this roadmap AND the first
human build of the Phase 0–10 workspace. No planning artifact claims any
implementation, run, or verification status.

## 4. Decision register (E-decisions)

Open decision points arising in phases 11–20 are registered per phase in the
phase `plan.md` and carry IDs `E-1`, `E-2`, ... scoped per phase (e.g.,
P14-E-1). Each entry records: the question, the options, a DEFAULT /
RECOMMENDATION label, and what evidence would close it. **Agents never close
an E-decision.** They implement the recommended default only when a subtask
explicitly says so, and record the dependence. Program-level decisions that
belong to the human owner (build-vs-buy budgets, vendor selection, market
jurisdiction entry, certification order) are always E-decisions with the
human named as closer.

## 5. Human gates

Every lab run, CI run, benchmark figure, scan result, signature, sign-off,
site, customer, or date is a HUMAN GATE INPUT recorded UNRUN / EMPTY /
UNSIGNED / NOT-STARTED. Pre-registration discipline (X-8) carries over:
thresholds are registered before any run that could fill them, and dynamic
results are kept structurally separate. §13.3 semantics carry over: zero
observed violations from a finite test set is a test outcome, never a
guarantee.

## 6. Architecture rules inherited from Phases 0–10

1. DDD + hexagonal + modular monolith; the dependency rule is enforced by
   `python -I tools/check_dependency_rule.py` and must stay green after every
   implementation batch. Domain depends on nothing infrastructure; adapters
   depend on ports; apps are composition roots.
2. New crates are created only when they have real code, clear ownership, and
   a stable dependency direction (AGENTS.md §1); each addition updates the
   workspace and the dependency-rule checker additively.
3. **ADR-010 holds and extends:** research/forecast/optimization modules have
   no dispatch authority. New intelligence features (forecasting, degradation
   pricing, health scoring) produce inputs, records, and proposals — never
   commands. MARL live commands remain PROHIBITED, not deferred.
4. The never-granted permission set (`dispatch.authorize`, `dispatch.issue`,
   `secret.read`, `network.any`) stays byte-identical; new protocol and
   market adapters receive dispatch reach only through the command lifecycle.
5. Solver-class discipline holds: HiGHS LP/QP/MIP, Clarabel convex conic,
   no MISOCP/MINLP creep; degradation costs enter the optimizer only as
   linear/piecewise-linear data, never as new problem classes.
6. Kernel patterns carry over: named errors, closed vocabularies, BTreeMap
   determinism, injected Clock, sealed construction, CRC-32 journals,
   restart-surviving checkpoints. Production-grade crypto in new security
   surfaces (audit chain, signing) is a DEPLOYMENT REQUIREMENT — test-grade
   digests (FNV-1a) remain labeled test-grade and are never claimed as
   release signatures.
7. Simulation/shadow ceiling: every new dispatch-adjacent capability lands
   simulation-only until the operator-approved qualification chain exists.

## 7. Honesty rules for this program's documents

1. No capability claims about unrun code. The 0–10 base is WRITTEN-UNRUN and
   the first build may surface recorded defects (e.g., F-P10.07-5); plans
   reference these as known integration risks.
2. Every external claim in phase contexts cites a source from
   `00_GAP_ANALYSIS.md` §11 or is marked UNVERIFIED/ASSUMPTION.
3. Protocol conformance, certification (OpenADR, CSIP, OCPP, ISASecure,
   SOC 2), and market qualification are human-gated certification processes;
   plans may prepare evidence and test suites but never claim conformance.
4. Product non-promises (§0.3 of the foundations: no hard real-time, no
   protection-grade control, no AC-OPF global optimality, no autonomous
   field dispatch) are inherited and extended to every new feature.

## 8. File conventions

- Phase directory names: `phase-<NN>-<short-slug>/` (e.g.,
  `phase-11-operational-robustness/`).
- Per phase exactly three planning files: `context.md`, `plan.md`,
  `ledger_phase_NN.md`. Implementation-time records, HANDOVER, CHECKLIST,
  and close records are added later under the same directory.
- `records/` subdirectories are created at implementation time.
- Citations to the gap analysis use `[S-<n>]` IDs defined once in
  `00_GAP_ANALYSIS.md` §11.
- Tables keep consistent pipe counts; section references use `§N`.
- US spelling ("license"); no generated artifacts committed.
