# Research — Phase 5: Operator Console and Simulator Integration

- **Checked:** 2026-11-07 (report research cut-off date; AGENTS.md §7).
- **Rule:** primary sources only (official docs, project repositories, PyPI/npm
  release pages). A version is listed only if a primary source was read. Anything
  not verified is labelled **ASSUMPTION** or written as an open verification task.
- **Honesty note:** the React/Vite/TypeScript versions below rest on secondary
  sources (dated third-party round-ups) because the official release pages were
  not re-fetched in this pass. They are recorded as **secondary-source findings
  to re-verify at scaffold time** (open task 1), not as primary-verified facts.
  Do not pin the console to them without that re-check.

## 1. HELICS — co-simulation orchestration

| Item | Finding |
|---|---|
| Latest version verified | **3.7.0** — released 25 August 2026 (project CHANGELOG on GitHub main: "3.7.0 - 2026-08-25", major release with improved broker startup controls, C shared-library support for querying broker/core status, and third-party library updates). Prior: 3.6.1 (21 February 2025), 3.6.0 (9 December 2024). |
| Source | https://github.com/GMLC-TDC/HELICS/blob/main/CHANGELOG.md ; https://docs.helics.org/en/latest/ ; report [R16] |
| Role (§12.4, §19.1) | Co-simulation orchestration across simulators, communication networks, market and end-use models. **It coordinates federates; it does not replace their physics models.** |
| License | BSD-3-Clause |
| Notes relevant to Phase 5 | HELICS is C++20 with Python (`pyhelics`/`helics` PyPI package), C, Java, MATLAB, and Julia bindings. The PyPI `helics-apps` package is archived and replaced by `pyhelics` — do not install `helics-apps` for new work (its final 3.7.0 upload is a shim to `pip install helics`). A 3.8 release was described in a secondary source as "on the roadmap for late 2026" — that is **not verified** and is not planned against. |
| Date checked | 2026-11-07 |

## 2. React — operator console UI framework

| Item | Finding |
|---|---|
| Latest version (secondary source) | **19.3.1** — reported shipped 14 October 2026, following 19.3.0 (9 September 2026); maintenance releases 19.0.9, 19.1.10, 19.2.9 shipped alongside. An earlier dated round-up reported 19.2.8 (21 July 2026). **Status: secondary-source only — re-verify against https://react.dev/versions and the React GitHub releases before pinning (open task 1).** |
| Source (secondary) | Dated third-party round-ups (tech-insider.org, November 2026). Primary source to check at scaffold time: https://react.dev/versions ; https://github.com/facebook/react/releases |
| Role (§7.5) | React + TypeScript console; official scaffold path is Vite's `react-ts` template. |
| License | MIT |
| Notes relevant to Phase 5 | React 19 removed `propTypes` behaviour (silently ignored) and added Actions, `use()`, ref-as-prop, and the React Compiler (1.0) in the 19.2 line per secondary sources — treat feature details as secondary until verified against react.dev. The report (§7.5) recommends React + TypeScript provisionally; the choice is revisit-triggered if accessibility or bundle constraints cannot be met (§25.1). |
| Date checked | 2026-11-07 (secondary sources) |

## 3. TypeScript and Vite — console toolchain

| Item | Finding |
|---|---|
| TypeScript (secondary source) | **7.0** reported as the current stable release in early August 2026, with a Go-native compiler (8–12x faster full builds per the secondary source). TypeScript 5.9 was reported as the current stable 5.x family earlier in 2026. **Status: secondary-source only — re-verify against https://www.typescriptlang.org/ and https://github.com/microsoft/TypeScript/releases before pinning (open task 1).** A known tooling friction reported in secondary sources: `typescript-eslint` peer-dependency caps may lag a TypeScript major — verify the lint toolchain combination at scaffold time rather than assuming it works. |
| Vite (secondary source) | **8.x** reported in use with React 19.2/19.3 and TypeScript 7 in dated 2026 round-ups (Vite 8.2.1 named in one tutorial; Vite 7.x named as the standard scaffold in another). **Status: secondary-source only — re-verify against https://vite.dev/ and https://github.com/vitejs/vite/releases before pinning (open task 1).** |
| Source (secondary) | Dated third-party round-ups (tech-insider.org, copyprogramming.com, August–November 2026). |
| Role (§7.5) | Vite for build tooling; TypeScript for the console. `create react app` is deprecated for new apps (secondary-source consensus; the report's recommendation of Vite stands on its own). |
| License | TypeScript: Apache-2.0. Vite: MIT. |
| Date checked | 2026-11-07 (secondary sources) |

## 4. TanStack Router / Query — route and data layers

| Item | Finding |
|---|---|
| Status | **ASSUMPTION — not verified in this pass.** The report (§7.5) recommends "TanStack Router and Query or equivalent". No version was verified against the TanStack release pages in this pass. Open task for P5.01: verify the current TanStack Router and TanStack Query versions from https://tanstack.com/ at scaffold time and record them, or record the chosen equivalent with its version and reason. |
| License | MIT (TanStack projects). Confirm at verification time. |

## 5. Simulator versions reused in Phase 5 (verified in Phase 4 RESEARCH.md)

| Item | Verified version (2026-11-07) |
|---|---|
| pandapower (grid adapter) | 3.5.6 (7 October 2026) — https://pypi.org/project/pandapower/ |
| OpenDSS (candidate second provider) | 11.0.0.1 "Charlottesville" (30 January 2026) — SourceForge announcement; license not re-verified |
| GridLAB-D (candidate second provider) | v5.2 (GitHub release; exact date not claimed) |
| PyPSA (planning only, not a distribution validator) | 1.4.0 (9 October 2026) |
| HELICS | 3.7.0 (25 August 2026) — see §1 above |

Full records, sources, and caveats are in `ledger/phase-04-grid-feasibility-dispatch/RESEARCH.md`. Do not copy a version from this table without its caveat.

## 6. What was deliberately NOT verified (open verification tasks)

1. **React / TypeScript / Vite current versions** — re-verify against the
   official release pages (react.dev/versions, typescriptlang.org,
   vite.dev, and the three GitHub release pages) at scaffold time (P5.01).
   Record the pinned versions and update this file; do not pin from the
   secondary sources cited above.
2. **TanStack Router/Query versions** — verify from https://tanstack.com/ at
   scaffold time, or record the chosen equivalent (P5.01).
3. **`pyhelics` binding version for HELICS 3.7.0** — confirm the PyPI
   `helics`/`pyhelics` release matching HELICS 3.7.0 before P5.16
   implementation; the archived `helics-apps` package must not be used.
4. **HELICS 3.8** — a secondary source mentions it as a late-2026 roadmap
   item; not verified, not planned against. Re-check only if the 3.7.0 line
   proves unsuitable.
5. **IEEE feeder licensing/availability** — confirm per feeder before use
   (§19.3, P5.20); no feeder is claimed available here.
6. **D4 license audit** — React (MIT), Vite (MIT), TypeScript (Apache-2.0),
   HELICS (BSD-3-Clause) are recorded as found/reported; the formal D4 audit
   is still OPEN and owned by legal + engineering.

## 7. Discipline restated for implementers

- HELICS coordinates federates; it does not replace their physics models.
- A steady-state power-flow tool is not evidence for transient frequency stability.
- The console is a view: it never defines authoritative permissions and never
  directly issues device commands (§7.5).
- A simulated value is labelled simulated; an acknowledgement is acceptance,
  not physical delivery.
