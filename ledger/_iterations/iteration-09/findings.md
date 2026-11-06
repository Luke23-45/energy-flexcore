# Iteration 9 findings — Research freshness / versions

Reviewer: iterations 7–10 subagent. Dates checked: 6–7 November 2026.
Scope: phase-10 topics first (SBOM/license tools, NIST SP 800-82 Rev. 4, FERC Order 2222,
IEEE 1547), then spot-checks of the riskiest claims across all phases' `RESEARCH.md`
(Wasmtime CVEs, Rust 1.100, pandapower 3.5.6, HiGHS 1.15.1, Clarabel 0.11.1, OpenEMS
2026.11.0, PyTorch/PyG skew, NATS CVE-2026-33222).
Method: `browser.search` + `browser.open` on primary sources only (NIST CSRC, FERC tracker
PDF, IEEE SCC21 table, ISO-NE readiness page, tool release pages, PyPI, Zenodo). A claim
counts as confirmed only when the primary source was fetched or its release page read;
secondary-source items are labelled as such. Cut-off: 7 November 2026 — nothing after
that date is claimed.

Severity scale: Critical = a wrong version/date that would make a release decision unsafe;
High = a wrong claim a subtask would act on; Medium = stale versions or a wrong detail
that the "re-verify at time of use" rule would catch, but which should not have been
left stale; Low = precision polish or an already-flagged open verification.

Counts: **High 0, Medium 4, Low 4. Critical 0.**

## (a) Missing subtasks

None. No research gap requires a new subtask; the corrections below are content updates
inside existing subtasks/files.

## (b) Issues / errors / contradictions — phase-10 RESEARCH.md (patched)

### M-9.1 — SBOM/license tool versions in §4 far behind the tools' own releases
- **Severity: Medium.**
- **File:** `ledger/phase-10-v1-release-hardening/RESEARCH.md` §4 (no subtask ID — research
  table feeding **P10.19**).
- §4 listed floors "≥1.30 / ≥0.100 / ≥2.2" and Trivy v0.72.0 "cited in the October 2025
  benchmark study". The floors are not wrong, but they are a year behind what P10.19 would
  actually run. Re-verified 7 Nov 2026 against the tools' own release pages:
  Syft **v1.56.0** (latest published, referenced in anchore/oss-docs#325 opened 3 Nov 2026;
  v1.55.0 released 20 Oct 2026), Grype **v0.121.0** (20 Oct 2026), Trivy **v0.76.1**
  (21 Oct 2026; v0.76.0 14 Oct 2026), OSV-Scanner **v2.7.0** (12 Oct 2026), cdxgen
  **13.5.0** (23 Oct 2026). ORT still has no version pinned by the sources consulted.
- **Patch applied:** §4 table updated with the re-verified versions and release dates; the
  "re-verify at time of use" rule kept (it is a P10.19 requirement, not a formality).

### M-9.2 — Trivy supply-chain compromise detail missing from the §4 caution note
- **Severity: Medium.**
- **File:** `RESEARCH.md` §4 (feeds **P10.19**).
- §4 noted only that "on 19 March 2026 the aquasecurity/trivy-action was compromised".
  The actionable detail was absent: a malicious Trivy v0.69.4 release and 76 of 77
  `trivy-action` tags were force-pushed (CVE-2026-33634, CISA KEV 26 Mar 2026); Trivy
  releases v0.27.0–v0.69.1 were deleted; v0.70.0 (17 Apr 2026) was the first clean
  post-incident release; `trivy-action` v0.36.0 (22 Apr 2026) was the first clean action
  release. A pipeline pinned to a deleted or compromised tag would fail — or worse, run
  compromised code — and §4 gave P10.19 no way to know.
- **Patch applied:** §4 note extended with the CVE, KEV date, deleted-release range, and
  first-clean versions, plus the instruction to re-pin before P10.19 runs. Two
  secondary-source reports are recorded as such, not as verified fact: Syft v1.55.0
  binaries built with Go 1.26.8 carrying stdlib vulnerabilities, and cdxgen 13.5.0/12.8.9
  remediating a bundled-Node Brotli flaw (CVE-2026-111497, CISA KEV 27 Oct 2026,
  remediation due 10 Nov 2026).

### L-9.1 — EU CRA date imprecise ("December 2027")
- **Severity: Low.**
- **File:** `RESEARCH.md` §4 (feeds **P10.19**).
- Full CRA obligations start **11 December 2027** (Art. 14 reporting duties began
  11 September 2026: 24 h early warning / 72 h notification / 14-day final report via the
  ENISA Single Reporting Platform). Sources: TuxCare, SOTA (9 Sep 2026), Vanta.
- **Patch applied:** "December 2027" → "11 December 2027".

### L-9.2 — CycloneDX 1.7 / SPDX 3.1 RC not noted; BSI minimum unchanged
- **Severity: Low.**
- **File:** `RESEARCH.md` §4 (feeds **P10.19**).
- CycloneDX **1.7** published late 2025 (ECMA-424 2nd edition, Dec 2025); SPDX **3.1 RC**
  published Jan 2026 (2nd RC in development Oct 2026); SPDX ISO/IEC 5962:2021 codifies
  2.2.1, not 3.x. BSI TR-03183-2 v2.1.0 (Aug 2025) requiring CycloneDX 1.6+ or SPDX 3.0.1+
  is confirmed by multiple sources and remains the minimum — the newer versions are noted,
  not required.
- **Patch applied:** §4 formats paragraph extended.

## (c) Re-verified claims — confirmed, no change

| Claim | Source (primary) | Date checked | Status |
|---|---|---|---|
| NIST SP 800-82 Rev. 4: initial public draft published 21 Sep 2026; comment period open through 30 Nov 2026; Rev. 3 (Sep 2023, 28 Sep 2023) remains the current final; draft restructured around CSF 2.0 Govern Function, expanded OT coverage, IR 8286r1 alignment, new RMF appendix | https://csrc.nist.gov/pubs/sp/800/82/r4/ipd ; NIST news page 2026-09-21 | 2026-11-07 | Confirmed. Note (Low, not patched): one search result mentions appendices C/D becoming web resources and E/F separate documents in the final — not verified. |
| FERC 2222: NYISO full implementation extended to 30 Apr 2027 (NYISO requested 15 Apr 2026; original 31 Dec 2026; FERC accepted 30 Jul 2026) | https://cuswebsite.blob.core.windows.net/2222tracker/Tracker-Report-July-2026.pdf (direct fetch, crawled 2026-11-06) | 2026-11-06 | Confirmed. General web search still surfaces the stale Dec 2026 date — this reinforces P10.26's region-specific-claim rule. |
| FERC 2222: ISO-NE effective 1 Nov 2026 ("participants can begin to register DERAs"; energy + ancillary; capacity aligned to FCA 19 / 1 Feb 2027 for the 2028/29 capacity year) | ISO-NE readiness page, last update 11/04/2026 | 2026-11-07 | Confirmed. |
| FERC 2222: PJM full energy/ancillary model effective 1 Feb 2028 | Tracker digest, July 2026 | 2026-11-06 | Confirmed (energy date). Capacity date remains "verify" — sources differ; one digest says proposed 1 Feb 2027 for the 2028/29 delivery year. |
| FERC 2222: MISO Phase 1 by 1 Jun 2027, Phase 2 by 1 Jun 2029 | Tracker digest, July 2026 | 2026-11-06 | Confirmed. |
| IEEE 1547 series: 1547-2018 valid, expires 2028, revision in ballot; 1547a-2020 valid to 2032; 1547.1-2020 valid to 2030, P1547.1a in ballot; 1547.2-2023 and 1547.3-2023 valid to 2033; 1547.9-2022 valid to 2032; 1547.4-2011, 1547.6-2011, 1547.7-2013 in revision; P1547.10 new project in progress | https://sagroups.ieee.org/scc21/ieee-1547-series/ (crawled 2026-11-06 19:39 UTC, "as of April 2026"); IEEE SA page shows active project P1547 | 2026-11-06 | Confirmed exactly. |
| Rust 1.100 planned 12 Nov 2026 — not yet stable on 7 Nov 2026; "do NOT pin 1.100" | Rust release pages; current stable 1.99.1 (15 Oct 2026; 1.99.0 on 1 Oct 2026) | 2026-11-07 | Confirmed. Secondary note: 1.100 adds Cargo dependency cooldowns (`global-min-publish-age`) and expected above-usual breakage (never-type stabilization, Cargo target-dir layout) — secondary sources only. |
| pandapower 3.5.6 still latest | https://pypi.org/project/pandapower/ (release files 7 Oct 2026) | 2026-11-07 | Confirmed. |
| HiGHS / highspy 1.15.1 still latest | https://pypi.org/project/highspy/1.15.1/ (released 2 Jul 2026) | 2026-11-07 | Confirmed. |
| Clarabel.rs 0.11.1 still latest Rust release | README badge v0.11.1 | 2026-11-07 | Confirmed. Do not confuse with R package clarabel 0.11.3 (CRAN, 23 Aug 2026), which tracks Clarabel.rs 0.11.1 exactly. |
| OpenEMS 2026.11.0 | https://zenodo.org/records/23834676 (published 31 Oct 2026) | 2026-11-07 | Confirmed. Low note (deferred): release notes say legacy Energy Scheduler v1 was removed and ENTSO-E tariff components migrated to the new spot-tariff system — parent to check whether any P6 lab config relied on it; not verified. |
| PyTorch 2.15 vs PyG 2.8.0 skew persists | PyTorch 2.15 GA announcement 28 Oct 2026 (3,117 commits, 583 contributors; CUDA 13.2 default wheel, 13.4 alongside; Python 3.11–3.15); PyG 2.8.0 (5 Jun 2026) install docs top out at torch 2.12.0 wheels | 2026-11-07 | Confirmed, still open, already flagged in P8 (R8-C3). PyTorch 2.15 first patch expected Nov 2026. |

## (d) Stale research / corrections needed in OTHER phases (recorded precisely, NOT edited)

### M-9.3 — P7 RESEARCH.md §1: Wasmtime CVE affected-range table lumps both flaws; third flaw missing
- **Severity: Medium.**
- **File:** `ledger/phase-07-*/RESEARCH.md` §1 (Wasmtime CVEs). Do NOT edit here — parent to apply.
- Verified: CVE-2026-115889 (bit-packing, all architectures) and CVE-2026-115893 (aarch64
  `br_table`), both CVSS 9.9, disclosed 19 Oct 2026, fixed in 36.0.19, 48.0.7, 49.0.4,
  50.0.0 (50.0.0 released 20 Oct 2026; 50.0.1/49.0.5/48.0.8 on 29 Oct 2026).
- **Correction:** CVE-2026-115889 was fixed on the main line already in **43.0.0 (Mar 2026)**;
  releases 43.0.0–47.x are affected ONLY by 115893, not 115889. P7's affected-range table
  lumps both flaws — split the ranges. **Missing from P7:** third flaw GHSA-m9f7-9w4p-gv38
  (symlink rename, macOS/FreeBSD, fixed 36.0.18/48.0.6/49.0.3, no CVE assigned) — add it.

### M-9.4 — P1/P2 RESEARCH.md: NATS CVE-2026-33222 fix-version wording partly wrong; sibling CVE missing
- **Severity: Medium.**
- **File:** `ledger/phase-01-*/RESEARCH.md` and `ledger/phase-02-*/RESEARCH.md` (NATS CVE
  note). Do NOT edit here — parent to apply.
- **Correction:** the fix landed in **2.11.15 and 2.12.6 (Mar 2026, GHSA-9983-vrx2-fg9c,
  CVSS 4.9, JetStream stream-restore auth bypass)** — NOT in 2.15.2. v2.15.2 was published
  **26 Oct 2026** (not 5 Nov 2026 as P1/P2 state; 5 Nov appears to be a secondary-source
  crawl date). **Missing:** sibling **CVE-2026-33223** (Nats-Request-Info header spoof,
  CVSS 6.4, same fix versions) — add it. The JetStream restore surface is directly
  relevant to P10.11; the P10.11 patch (scoped restore authorization, iteration 7) already
  cites this surface.

## (e) Open verifications (not errors — tasks, not facts)

- **L-9.3 (Low):** SPP Q2 2030 + SPP final approval by letter order 21 May 2026, and CAISO
  fully implemented effective 1 Nov 2024 — NOT independently re-checked beyond the cited
  source (tracker PDF lines beyond 271 unread). Record as open verification.
- **L-9.4 (Low):** OpenADR 3.1 certification-profile availability — still not verified.
  Search shows OpenADR spec 3.1.0 exists (webinar "updates from spec version 3.0.1 to
  3.1.0"; GB flex profile extended to v3.1; Austrian DSS profile against OpenADR 3 v1.1.0
  uses 3.1.1 in the URL path) but no certification-profile availability found. P10
  RESEARCH §6 correctly leaves it open; reframe the open task to ask about certification
  profiles specifically, not the spec version. No change made.

## (f) Suggestions

- The §4 "re-verify at time of use" rule saved M-9.1 from being worse than Medium — the
  floors were not wrong. Keep the rule, but the table itself should be re-verified at each
  release, not only at time of use; the new §8 log gives P10.19 a dated baseline to diff
  against.
- The P7/P1/P2 corrections above (M-9.3, M-9.4) are the only Medium items left open after
  this iteration; both are precise one-paragraph fixes for the parent.

## Applied vs deferred log

| Item | Disposition |
|---|---|
| M-9.1 | **Applied** — RESEARCH.md §4 tool table (Syft 1.56.0, Grype 0.121.0, Trivy 0.76.1, OSV-Scanner 2.7.0, cdxgen 13.5.0) |
| M-9.2 | **Applied** — RESEARCH.md §4 Trivy CVE-2026-33634 incident detail |
| L-9.1 | **Applied** — RESEARCH.md §4 CRA "11 December 2027" |
| L-9.2 | **Applied** — RESEARCH.md §4 CycloneDX 1.7 / SPDX 3.1 RC note |
| (new §8) | **Applied** — RESEARCH.md §8 iteration-9 re-verification log table added |
| M-9.3 | **Deferred — other phase** — P7 RESEARCH.md §1 Wasmtime correction recorded above; not edited (out of patch scope) |
| M-9.4 | **Deferred — other phase** — P1/P2 RESEARCH.md NATS CVE correction recorded above; not edited (out of patch scope) |
| L-9.3 | **Deferred — open verification** — SPP/CAISO FERC 2222 dates not independently re-checked; not asserted as fact |
| L-9.4 | **Deferred — open verification** — OpenADR 3.1 certification profiles still unverified; P10 RESEARCH §6 wording already correct |
