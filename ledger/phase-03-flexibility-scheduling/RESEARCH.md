# Research — Phase 3: Flexibility and Deterministic Schedule

- **Checked:** 2026-11-07 (report research cut-off date; AGENTS.md §7).
- **Rule:** primary sources only (official docs, project repositories, PyPI/crates.io
  release pages). A version is listed only if a primary source was read. Anything
  not verified is labelled **ASSUMPTION** or written as an open verification task.
  Solver classes are never conflated: HiGHS is LP/QP/MIP, not SOCP; Clarabel is
  convex conic, not MISOCP; IPOPT is local, not global.

## 1. HiGHS — LP / QP / MIP (Tier 1)

| Item | Finding |
|---|---|
| Latest version verified | **1.15.1** — highspy 1.15.1 released 2 July 2026 (PyPI release files). The HiGHS project lists LP, MIP, and QP support; it does not list SOCP. |
| Source | https://pypi.org/project/highspy/1.15.1/ ; https://highs.dev/ ; report [R28] |
| Class | LP, convex QP, MIP. **Not SOCP.** |
| License | MIT (project repository https://github.com/ERGO-Code/HiGHS) |
| Notes relevant to Phase 3 | v1.14.0 (April 2026) added HiPO interior-point LP/QP and HiPDLP (CPU/GPU PDLP); v1.15.0 (June 2026) added the first variant of the parallel MIP solver. The Python binding is `highspy`. Determinism: record thread count and solver options per run (P3.19); parallel MIP behaviour must be pinned or disabled in determinism tests until verified. **Gap found 7 November 2026 (iteration-02 review):** the 1.15.1 pin above is the HiGHS solver version as shipped through `highspy` (Python). No Rust HiGHS binding version was researched in this ledger — the Rust `highs` crate version is an open task for P3.17 (§6 task 6). P3.17 must record the binding choice and pin the binding version before use; do not cite 1.15.1 as the Rust binding's version. |
| Date checked | 2026-11-07 |

## 2. Clarabel — convex conic, including SOCP (Tier 2)

| Item | Finding |
|---|---|
| Latest version verified | **Clarabel.rs 0.11.1** (docs.rs, crate `clarabel` 0.11.1; CHANGELOG entry 0.11.1 dated 2025-11-06). The R package `clarabel` 0.11.3 (CRAN, 23 August 2026) tracks Clarabel.rs 0.11.1 exactly — do not confuse the R package version (0.11.3) with the Rust solver version (0.11.1). No Clarabel.rs release newer than 0.11.1 was found on docs.rs or in the project CHANGELOG as of this check. |
| Source | https://docs.rs/crate/clarabel/latest/source/README.md ; https://raw.githubusercontent.com/oxfordcontrol/Clarabel.rs/main/CHANGELOG.md ; https://clarabel.org/stable/ ; report [R14] |
| Class | LP, QP, SOCP, SDP, exponential and power cones (convex conic). **No integer variables — not MISOCP.** |
| License | Apache-2.0 |
| Notes relevant to Phase 3 | CVXPY uses Clarabel as its default solver for LP/SOCP since CVXPY 1.5 (secondary-source survey; treat the "default since 1.5" detail as corroborated by the CVXPY docs, not independently re-verified here). Persistent solver API for warm starts was added in the 0.11.x line. The Rust crate is a native implementation, not a binding, which fits the Rust control plane (§7.3). |
| Date checked | 2026-11-07 |

## 3. CVXPY — modelling layer (not a solver)

| Item | Finding |
|---|---|
| Latest version verified | **1.9.3** — released 19 September 2026 (PyPI; release history 1.9.0 18 May 2026, 1.9.1 26 May 2026, 1.9.2 22 June 2026, 1.9.3 19 September 2026). |
| Source | https://pypi.org/project/cvxpy/ ; https://www.cvxpy.org/ |
| Class | Python-embedded modelling language for convex optimization. **CVXPY is not a solver**; capabilities depend on the selected backend (Clarabel, SCS, OSQP, HiGHS). |
| License | Apache-2.0 |
| Notes relevant to Phase 3 | CVXPY 1.8 onward uses HiGHS as the default MILP solver (per PyPI project description and release notes; the exact version boundary was corroborated across PyPI pages). Use CVXPY only as an experiment/modelling path in the Python worker; the production Tier 1/Tier 2 path goes through the `Optimizer` port with pinned solver versions, and the CVXPY version (if used) must be recorded in the §12.6 result record alongside the backend solver version. |
| Date checked | 2026-11-07 |

## 4. IPOPT — local nonlinear optimization (Tier 4 boundary only)

| Item | Finding |
|---|---|
| Latest version verified | **3.14.20** — COIN-OR Ipopt releases/3.14.20 (FreshPorts port updated 3.14.19 → 3.14.20 on 12 September 2026; DeepWiki mirror of the Ipopt source records current version 3.14.20; GAMS third-party list records Ipopt 3.14.20). The Ipopt ChangeLog page lists 3.14.19 (30 July 2025) as its latest dated entry at the time of that crawl; 3.14.20 is verified via the package/source records above, not via the ChangeLog page. **Corroborated 7 November 2026 (iteration-02 review):** FreshPorts commit record ("math/ipopt: update 3.14.19 → 3.14.20", 12 Sep 2026) and the GAMS third-party table both list 3.14.20 — the version's existence is confirmed; the GitHub release-notes read in §6 task 4 remains a build-time check. |
| Source | https://coin-or.github.io/Ipopt/ ; https://www.freshports.org/math/ipopt/ ; Python wrapper `cyipopt` 1.7.0 (20 April 2026) at https://pypi.org/project/cyipopt/ |
| Class | Large-scale nonlinear optimization (interior-point, filter line search). **Local solve only; no global-optimality guarantee.** Continuous variables natively. |
| License | Eclipse Public License (EPL) |
| Notes relevant to Phase 3 | IPOPT is **not** a Phase 3 scheduler solver. It is recorded here only so the Tier 4 boundary (P3.20) is honest: nonlinear AC-OPF is Stage 8 research, and any future IPOPT use must state its local-optimality limitation. EPL license must be included in the D4 license audit (unlike the MIT/Apache-2.0 solvers above). |
| Date checked | 2026-11-07 |

## 5. Supporting numerical stack (for the Python worker)

| Item | Finding / status |
|---|---|
| NumPy / pandas | Standard worker-local data handling (§7.4). No version pinned in this ledger — pin at implementation from the worker lockfile and record it in the §12.6 result record. **ASSUMPTION:** a current 2026 NumPy/pandas pair is available for Python 3.11+; verify at build time. |
| Python version | AGENTS.md requires Python 3.11+ for repo tooling. The worker's Python version must be pinned in its container image and recorded per optimization run. |
| Rust toolchain | Pinned via `rust-toolchain.toml` (§8.1) at implementation time; no version claimed here. |

## 6. What was deliberately NOT verified (open verification tasks)

1. **HiGHS parallel-MIP determinism:** whether v1.15.x parallel MIP produces
   identical results run-to-run with a fixed thread count. Open task for P3.19:
   run the determinism test with parallel MIP on and off and record the outcome;
   do not assume determinism.
2. **Clarabel.rs release after 0.11.1:** no newer Rust release was found on
   docs.rs/CHANGELOG as of 2026-11-07. Re-check at implementation time; if a
   newer version exists, update this file and the pin — do not silently upgrade.
3. **CVXPY 1.9.x solver-default matrix:** the exact per-problem-class default
   solver table for 1.9.3 was not re-verified against cvxpy.org docs in this
   pass. Open task for P3.17/P3.18 if CVXPY is used as a modelling path.
4. **IPOPT 3.14.20 ChangeLog entry:** the dated ChangeLog page had not yet
   listed 3.14.20 at crawl time. **Update 7 November 2026 (iteration-02
   review):** the version's existence is now confirmed via the FreshPorts
   commit record (3.14.19 → 3.14.20, 12 Sep 2026) and the GAMS third-party
   list (see §4). The GitHub release-notes read
   (https://github.com/coin-or/Ipopt/releases) remains a build-time check,
   not an open version question.
5. **D4 license audit:** MIT (HiGHS), Apache-2.0 (Clarabel, CVXPY), EPL
   (IPOPT) are recorded here as found, but the formal D4 audit is still OPEN
   and is owned by legal + engineering — this file is input to it, not the
   audit itself.
6. **Rust HiGHS binding version (new, 7 November 2026, iteration-02
   review):** no Rust `highs` crate version was researched in this ledger.
   Open task for P3.17: research the Rust binding's current version from
   crates.io/docs.rs, pin it, and record the binding choice (highspy vs
   Rust binding) before any Tier 1 code is written.

## 7. Solver-class discipline (restated for implementers)

- HiGHS: LP / QP / MIP. Never describe it as an SOCP solver. Never route a
  conic constraint to it.
- Clarabel: convex conic (LP/QP/SOCP/SDP, exponential/power cones). Never route
  an integer-constrained problem to it; it has no integer support and is not
  MISOCP.
- CVXPY: modelling layer. Its capability is the capability of the backend
  solver actually selected; record both versions.
- IPOPT: local nonlinear. Never present an IPOPT result as globally optimal.
- AC power flow evaluates a network state; AC-OPF is a different problem and is
  not in this phase at all.
