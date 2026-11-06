# Research — Phase 4: Grid Feasibility and Dispatch Assurance

- **Checked:** 2026-11-07 (report research cut-off date; AGENTS.md §7).
- **Rule:** primary sources only (official docs, project repositories, PyPI release
  pages). A version is listed only if a primary source was read. Anything not
  verified is labelled **ASSUMPTION** or written as an open verification task.
- **Discipline:** AC power flow evaluates a network state; AC-OPF is a different
  problem. No tool below is claimed to do the other's job.

## 1. pandapower — primary power-flow provider

| Item | Finding |
|---|---|
| Latest version verified | **3.5.6** — released 7 October 2026 (PyPI release files; piwheels and pepy.tech corroborate 3.5.6, 7 October 2026). Prior releases: 3.5.5 (22 September 2026), 3.5.4 (8 July 2026). A 4.0.0 is listed as upcoming in the project changelog with no release date. |
| Source | https://pypi.org/project/pandapower/ ; https://pandapower.readthedocs.io/en/develop/about/changelog.html ; https://github.com/e2nIEE/pandapower ; report [R20] |
| Role (§12.4) | Python power-system analysis, balanced AC power flow, some three-phase/asymmetric calculations, OPF studies. Boundary: verify model and solver limitations for each case. |
| License | BSD-3-Clause (project repository). |
| Notes relevant to Phase 4 | pandapower is the recommended default primary provider (§25.2). Its three-phase/asymmetric support exists but is not the same as a dedicated unbalanced solver — the Phase 4 adapter must declare its scope (P4.02) and the D3 default (balanced radial) stands until a pilot requires otherwise. pandapower's own OPF capability is not used as the feasibility gate: the gate runs AC power flow and checks limits (§12.5). |
| Date checked | 2026-11-07 |

## 2. OpenDSS — distribution-system analysis (second-provider candidate)

| Item | Finding |
|---|---|
| Latest version verified | **11.0.0.1 "Charlottesville"** — announced 30 January 2026 (EPRI SourceForge news). The EPRI "What's new" page still lists 10.0.0.1 "Columbus" as its headline at crawl time, so the 11.0.0.1 finding rests on the SourceForge release announcement, corroborated by the SourceForge files listing. |
| Source | https://sourceforge.net/p/electricdss/news/2026/01/opendss-version-11001---charlottesville---released/ ; https://sourceforge.net/projects/electricdss/files/OpenDSS/ ; https://opendss.epri.com/ ; report [R22] |
| Role (§12.4) | Distribution-system analysis, unbalanced feeder studies, DER simulation. Boundary: use through a defined simulator adapter. |
| License | **ASSUMPTION — not verified in this pass:** OpenDSS is distributed by EPRI under a BSD-style license historically; the exact license text for 11.0.0.1 was not re-verified here. Open verification task for the D4 audit: confirm the license from the 11.0.0.1 distribution README before any adoption. |
| Notes relevant to Phase 4 | OpenDSS is the natural second provider if a named test case needs unbalanced feeder studies (P4.01 rule; IEEE 13-bus is the likely trigger, §19.3). It is not the primary provider in this phase, and no OpenDSS adapter is built unless that case is named. |
| Date checked | 2026-11-07 |

## 3. GridLAB-D — detailed distribution simulation (second-provider candidate)

| Item | Finding |
|---|---|
| Latest version verified | **v5.2** — GitHub release `gridlab-d/gridlab-d` v5.2 (release branch `release/5.1` merged, version-number and three-phase/triplex fixes). The release page was crawled 84 days before this check with no newer release tag found in search; the exact v5.2 release date was not stated on the release page excerpt and is therefore **not claimed here**. |
| Source | https://github.com/gridlab-d/gridlab-d/releases/tag/v5.2 ; https://gridlab-d.readthedocs.io/ ; report [R21] |
| Role (§12.4) | Detailed distribution simulation, unbalanced three-phase power flow, end-use/DER models. Boundary: distribution and time-series studies; not automatically a fast transient-stability engine. |
| License | BSD-style (BSD-3-Clause family per project history). **ASSUMPTION:** exact license text for v5.2 not re-verified in this pass — confirm from the v5.2 distribution before any adoption (D4 audit task). |
| Notes relevant to Phase 4 | GridLAB-D is a candidate second provider for unbalanced/time-series cases only. A SourceForge discussion note records that the GitHub repository is current and the SourceForge copy is out of date, and that the `release/RC4.0` branch was the then-best stable line — treat branch advice from discussions as secondary and verify against the v5.2 release at implementation time. GridLAB-D is not a transient-stability engine by default (§12.4). |
| Date checked | 2026-11-07 |

## 4. PyPSA — energy-system / transmission planning (not a distribution validator)

| Item | Finding |
|---|---|
| Latest version verified | **1.4.0** — released 9 October 2026 (PyPSA release notes; piwheels lists 1.4.0, 2026-10-09; prior 1.3.0, 19 August 2026). |
| Source | https://docs.pypsa.org/stable/release-notes/ ; https://www.piwheels.org/project/pypsa/ ; https://github.com/PyPSA/PyPSA ; report [R23] |
| Role (§12.4) | Energy-system and transmission/planning optimization. Boundary: **not the sole distribution device-control validator**. |
| License | GPL-3.0-or-later for PyPSA itself (project repository). **Note for D4:** GPL licensing has different compatibility implications from the BSD/MIT/Apache stack — record in the D4 audit; do not assume Apache-2.0 compatibility. |
| Notes relevant to Phase 4 | PyPSA is used for transmission/planning studies (§19.1), not as the Phase 4 distribution feasibility validator. Its presence in this file is to prevent misuse: do not route a feeder feasibility check through PyPSA. |
| Date checked | 2026-11-07 |

## 5. Supporting tools referenced by the report (status, not adoption)

| Item | Finding / status |
|---|---|
| ANDES (dynamic stability, §12.4 [R24]) | Not a Phase 4 dependency. Dynamic-stability validation is a separate track (§19.1); do not use a steady-state power-flow tool as evidence for transient frequency stability. Version not verified in this pass — open task if the track is ever opened. |
| HELICS (co-simulation, §12.4 [R16]) | Version 3.7.0, 25 August 2026 (see Phase 5 RESEARCH.md for the verified record). HELICS coordinates federates; it does not replace their physics models. Not a Phase 4 feasibility provider. |
| IEEE test feeders (§19.3) | IEEE 13/34/123-bus distribution and 39/118-bus transmission cases are the named benchmark cases. Licensing and model availability must be confirmed per case before use — not assumed. No feeder files are claimed to be in hand. |

## 6. What was deliberately NOT verified (open verification tasks)

1. **OpenDSS 11.0.0.1 license text** — confirm from the distribution README (D4 audit input).
2. **GridLAB-D v5.2 exact release date and license text** — confirm from the GitHub release page at implementation time.
3. **GridLAB-D v5.2 vs RC4.0 branch status** — the discussion note about branches is secondary; verify which branch/tag is current when a GridLAB-D adapter is ever considered.
4. **pandapower 4.0.0** — listed as upcoming with no date; do not plan against it. Re-check at implementation time and record if released.
5. **IEEE feeder model availability/licensing** — confirm per feeder before use (§19.3); no claim is made here that any feeder file is available.
6. **PyPSA GPL compatibility (D4)** — legal to assess; this file only records the license as found.

## 7. Discipline restated for implementers

- AC power flow (pandapower `runpp`, or equivalent) evaluates injections. It is
  not AC-OPF. The feasibility gate never optimizes.
- A power-flow convergence failure is a result, not a bug to retry away.
- A model that does not cover part of the network cannot pass that part.
- pandapower's OPF feature, if present in the installed version, is not the
  Phase 4 gate and must not be wired in as one without a new decision.
