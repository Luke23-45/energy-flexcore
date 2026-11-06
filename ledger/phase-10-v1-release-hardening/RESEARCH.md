# Phase 10 — v1.0.0 Pilot-Ready Release Hardening / Closure: Research

**Checked:** 7 November 2026 (the report's research cut-off, §27)
**Rule:** Primary sources only (official docs, repos, standards bodies, regulators). Where a
primary source could not be verified, the item is recorded as an open verification task, not
as a fact. Versions and statuses below are as found on the check date; re-verify at the time
of use — standards, drafts, and tool versions move. Phase 10 re-checks the items below at
the time its subtasks run (notably P10.16 for NIST SP 800-82 Rev. 4 and P10.19 for tool
versions).

## 1. NIST SP 800-82 — Rev. 3 baseline, Rev. 4 draft (re-check due in P10.16)

**Status as checked 2026-11-07 (verified against the NIST CSRC primary page):**

- **SP 800-82 Rev. 3**, *Guide to Operational Technology (OT) Security*: final, published
  September 2023 (28 September 2023). Baseline for the Phase 9 assessment (P9.07).
- **SP 800-82 Rev. 4**: **initial public draft, published 21 September 2026; comment period
  open through 30 November 2026.** Draft, not a final standard.
- Rev. 4 draft changes: restructured around the NIST Cybersecurity Framework (CSF) 2.0
  (risk management reorganised around the Govern Function); expanded OT sector coverage
  (building automation and control systems, water/wastewater, food and agriculture, freight
  rail, maritime vessels, IIoT and cloud convergence); OT risk alignment with enterprise
  risk management (NIST IR 8286r1); new appendix on adopting the NIST Risk Management
  Framework (Appendix F); expanded asset-management, monitoring/detection, security-
  architecture (protecting system-management functions), and zero trust guidance.

**Phase 10 action (P10.16):** the comment period closes 30 November 2026, after this
ledger's cut-off. On the P10.16 review date, check the CSRC page: if a final Rev. 4 has
published, record its deltas against the Rev. 3 baseline and assess whether any delta
changes a control in this release; if it has not published, record that Rev. 3 remains the
baseline. Do not assume the final matches the draft.

**Sources:**

- NIST CSRC, SP 800-82 Rev. 4 initial public draft —
  https://csrc.nist.gov/pubs/sp/800/82/r4/ipd (verified by direct fetch 2026-11-07)
- NIST CSRC, SP 800-82 Rev. 3 final — https://csrc.nist.gov/pubs/sp/800/82/r3/final
  (report [R26])

## 2. FERC Order 2222 — implementation status (context for the release notes)

**Status as checked 2026-11-07:**

| RTO/ISO | Status |
|---|---|
| CAISO | Fully implemented; effective 1 November 2024 — the only RTO/ISO fully live as of early/mid-2026 |
| ISO New England | Energy and ancillary-services participation targeted/effective 1 November 2026 |
| NYISO | Full implementation extended to 30 April 2027 (FERC accepted 30 July 2026; original date 31 December 2026) |
| PJM | DER aggregation participation model expected effective 1 February 2028 (energy); capacity-market date — **verify at time of use, sources differ** |
| MISO | Phase 1 by 1 June 2027, Phase 2 by 1 June 2029 |
| SPP | Proposed second quarter of 2030 |
| All six RTOs/ISOs | Compliance filings approved by FERC (SPP final approval by letter order 21 May 2026) |

**Implication for this phase (P10.26):** the release notes must not claim universal market
qualification or Order 2222 compliance (§0.3, §2.3). Any market claim in the notes is
region-specific and tied to the Phase 9 evidence (P9.11).

**Sources:**

- FERC Order 2222 Fact Sheet — https://ferc.gov/media/ferc-order-no-2222-fact-sheet
  (report [R1])
- FERC Order 2222 & DER Policy and Implementation Report, July 2026 —
  https://cuswebsite.blob.core.windows.net/2222tracker/Tracker-Report-July-2026.pdf
- PNNL, "FERC Order 2222 Considerations for RERRAs and EDCs", May 2026 —
  https://www.pnnl.gov/sites/default/files/media/file/FERC%20Order%202222%20Considerations%20for%20RERRAs%20and%20EDCs.pdf

## 3. IEEE 1547-2018 — status and revision (context for the release notes and P10.16)

**Status as checked 2026-11-07 (IEEE SCC21 1547 series table, as of April 2026):**

- **IEEE 1547-2018**: valid; standard expires 2028; **revision in ballot**.
- **IEEE 1547a-2020** (Amendment 1): valid to 2032.
- **IEEE 1547.1-2020** (conformance test procedures): valid to 2030; **P1547.1a** in ballot.
- **IEEE 1547.2-2023** (application guide) and **IEEE 1547.3-2023** (cybersecurity guide for
  DER): valid to 2033.
- **IEEE 1547.9-2022** (energy-storage interconnection guide): valid to 2032.
- **IEEE 1547.4-2011, 1547.6-2011, 1547.7-2013**: in revision.
- **P1547.10** (DER gateway platforms): new project, in progress.

**Implication for this phase:** the in-ballot revision of 1547-2018 is a watch item for the
release notes (P10.26) and the post-release review (P10.30): when the revision publishes,
the pilot's interconnection basis (P9.12) must be re-checked.

**Sources:**

- IEEE SCC21, IEEE 1547 Series table — https://sagroups.ieee.org/scc21/ieee-1547-series/
- IEEE Standards Association, IEEE 1547-2018 — https://standards.ieee.org/ieee/1547/5915/
  (report [R18])

## 4. SBOM and license-scanning tools (for P10.19 — D4 closure)

**Formats:** SPDX (ISO/IEC 5962:2021, Linux Foundation) and CycloneDX (OWASP) are the two
machine-readable SBOM formats in practice. Germany's BSI TR-03183-2 v2.1.0 (August 2025)
requires CycloneDX 1.6+ or SPDX 3.0.1+, SHA-512 hashes, and license identifiers as SPDX
expressions. CycloneDX 1.7 was published in late 2025 (ECMA-424 2nd edition, December
2025) and SPDX 3.1 reached release-candidate stage in January 2026 (a second RC was in
development in October 2026); BSI TR-03183-2 v2.1.0 still requires CycloneDX 1.6+ or
SPDX 3.0.1+ as the minimum — the newer format versions are noted here, not required.
The EU Cyber Resilience Act (Regulation (EU) 2024/2847) requires a
machine-readable SBOM (Annex I, Part I, para. 2(c); Article 13 due diligence for
third-party components); Article 14 vulnerability-reporting duties began 11 September 2026,
with the full obligations from 11 December 2027.

**Tools — versions re-verified 7 November 2026 against the tools' own release pages
(see §8); still re-verify at time of use — this is a P10.19 requirement, not a
formality:**

| Tool | Role | Version as reported (source, date) |
|---|---|---|
| Syft (Anchore, Apache-2.0) | SBOM generation from lockfiles/images | v1.56.0 latest published as of 7 Nov 2026 (anchore/oss-docs#325, 3 Nov 2026); v1.55.0 released 20 Oct 2026; v1.46.0 cited in an October 2025 benchmark study |
| Grype (Anchore) | Vulnerability scanning on an SBOM | v0.121.0 released 20 Oct 2026 (re-verified 7 Nov 2026) |
| OSV-Scanner (Google) | Vulnerability scanning via the OSV database (NVD, GitHub Security Advisories, ecosystem feeds) | v2.7.0 released 12 Oct 2026 (re-verified 7 Nov 2026) |
| Trivy (Aqua) | Combined SBOM + vulnerability + misconfiguration scanning | v0.76.1 released 21 Oct 2026 (v0.76.0 14 Oct 2026); v0.72.0 cited in the October 2025 benchmark study |
| cdxgen (OWASP) | CycloneDX generation | v13.5.0 released 23 Oct 2026 (re-verified 7 Nov 2026); strongest recall among OSS scanners in the October 2025 benchmark (42.96% recall; precision 29.78%–35.70%) |
| OSS Review Toolkit (ORT) | Comprehensive multi-ecosystem license scanning and policy evaluation | Current as of 2026 (no version pinned by the sources consulted) |
| Microsoft SBOM Tool | SPDX 2.2 generation | Cited in the October 2025 benchmark study |

**Caution from the benchmark study:** all six major tools achieved 100% compliance with
mandatory field presence while failing semantic completeness (e.g. Syft 11.47% precision /
13.89% recall in that study's measurement). A generated SBOM is not a verified SBOM:
P10.19 requires a completeness spot-check against the lockfiles. **Tool versions move
quickly; the versions above are as reported by the cited sources, not verified by this
check against the tools' own release pages. P10.19 must confirm the current release
version of each tool used against its own repository and record the version actually run.**
Also note: on 19 March 2026 the aquasecurity/trivy-action was compromised in a supply-chain
attack — pin CI actions to commit SHAs, not mutable tags (reported in the Augment Code
guide below). The incident detail, confirmed against the Trivy security advisory and
CISA KEV: a malicious Trivy v0.69.4 release and 76 of 77 `trivy-action` tags were
force-pushed (CVE-2026-33634, added to CISA KEV 26 March 2026); Trivy releases
v0.27.0–v0.69.1 were deleted, v0.70.0 (17 April 2026) was the first clean
post-incident release, and `trivy-action` v0.36.0 (22 April 2026) was the first clean
action release. Any pipeline pinned to a deleted or compromised tag must be re-pinned
before P10.19 runs. Secondary-source reports (not independently verified here): Syft
v1.55.0 binaries built with Go 1.26.8 carried stdlib vulnerabilities, and cdxgen
13.5.0 / 12.8.9 remediate a bundled-Node Brotli flaw (CVE-2026-111497, CISA KEV
27 October 2026, remediation due 10 November 2026) — P10.19 must check the tool
versions it actually runs against these advisories.

**Sources:**

- CRA Decoded, "CRA SBOM Tools: Syft, Grype, Snyk, Anchore (2026)" —
  https://cra-decoded.com/blog/posts/cra-toolchain-open-source-vs-commercial-2026/
- SOTA, "CRA SBOM Requirements 2026" —
  https://www.sota.io/blog/cra-sbom-software-bill-of-materials-eu-compliance-2026
- Augment Code, "SBOM for Agent-Driven Pipelines" —
  https://www.augmentcode.com/guides/sbom-for-agent-driven-pipelines
- Anchore Syft repository — https://github.com/anchore/syft (verify the current release here
  at time of use)
- OSS Review Toolkit repository — https://github.com/oss-review-toolkit/ort (verify the
  current release here at time of use)

**Open verification task (P10.19):** confirm the current Syft/Grype/Trivy/ORT release
versions and the SPDX/CycloneDX format versions against each tool's own repository before
recording them in the D4 closure record.

## 5. HIL testing practice (context for the consolidated reviews)

Verified in the Phase 9 research and carried here as background for P10.16–P10.17 (the
consolidated reviews rely on the Phase 9 HIL evidence; Phase 10 does not re-run HIL):

- IEEE 1547.1-2020 conformance test procedures; PHIL is the established method for
  ride-through tests against real inverters (real-time simulator + power amplifier).
- SunSpec System Validation Platform (SVP): open-source automated IEEE 1547.1-2020 test
  scripts (https://github.com/jayatsandia/svp_1547.1), driving OPAL-RT and Typhoon HIL.
- HIL platforms: OPAL-RT, RTDS/RSCAD, Typhoon HIL; CHIL and PHIL are distinct.

**Sources:** see Phase 9 `RESEARCH.md` §4 for the full entries (OPAL-RT article at
https://www.opal-rt.com/power-in-mind/volume-8/article-3/; Sandia/SIRFN paper at
https://psecommunity.org/wp-content/plugins/wpor/includes/file/2303/LAPSE-2023.17925-1v1.pdf).

## 6. OpenADR 3.0 — certification status (context for the adapter matrix, P10.01)

- OpenADR 3.0 is not backward compatible with 2.0 and does not replace 2.0a/2.0b.
- First OpenADR 3.0 certified products announced 24 March 2025 (E.ON Energy Networks —
  first VTN; EVoke Systems; Universal Devices — VEN). Only products through the OpenADR
  Alliance Certification Program may claim certified status.
- OpenADR 3.1 certification-profile availability was **not verified** in this check; confirm
  before naming 3.1 in the adapter matrix (P10.01).

**Sources:**

- OpenADR Alliance, first OpenADR 3.0 certified products —
  https://openadr.memberclicks.net/index.php?option=com_content&view=article&id=233:first-openadr-3-0-certified-products&catid=21:press-releases&Itemid=121
- OpenADR Alliance, OpenADR 3.0 introduction and certification —
  https://www.openadr.org/openadr-3-0 (report [R29])

## 7. Items NOT verified in this check

- Current release versions of Syft, Grype, Trivy, OSV-Scanner, and cdxgen — verified
  7 November 2026 (see §4 and §8). ORT still has no version pinned (open task for
  P10.19 — see §4).
- Whether NIST SP 800-82 Rev. 4 has published as final (open task for P10.16 — see §1).
- OpenADR 3.1 certification-profile status (open task for P10.01 — see §6).
- OCPP 2.1 device/fleet support for specific vendors; IEEE 2030.5/CSIP regional profile
  (carried from Phase 9; verify per vendor/region if named in the adapter matrix).
- Any pilot-customer-specific fact. All such facts are established inside the subtasks.

## 8. Iteration 9 re-verification log (7 November 2026)

Re-checked by the iterations 7–10 reviewer against the primary sources below. "Confirmed"
means the claim in this file still held on 7 November 2026; "Updated" means this file was
corrected in the same pass; "Open" means not independently re-verified — it is a task, not
a fact.

| Claim | Source checked | Date checked | Status |
|---|---|---|---|
| NIST SP 800-82 Rev. 4: initial public draft 21 Sep 2026; comment period through 30 Nov 2026; Rev. 3 (Sep 2023) remains the current final | https://csrc.nist.gov/pubs/sp/800/82/r4/ipd ; NIST news page 2026-09-21 | 2026-11-07 | Confirmed — no change. Note: one search result mentions appendices C/D becoming web resources and E/F separate documents in the final; not verified, low priority |
| FERC 2222: NYISO full implementation extended to 30 Apr 2027 (FERC accepted 30 Jul 2026) | https://cuswebsite.blob.core.windows.net/2222tracker/Tracker-Report-July-2026.pdf | 2026-11-06 | Confirmed — no change. General web search still surfaces the stale Dec 2026 date |
| FERC 2222: ISO-NE effective 1 Nov 2026 (energy + ancillary; capacity aligned to FCA 19 / 1 Feb 2027) | ISO-NE readiness page, last update 11/04/2026 | 2026-11-07 | Confirmed — no change |
| FERC 2222: PJM full energy/ancillary model effective 1 Feb 2028; capacity date verify | Tracker digest, July 2026 | 2026-11-06 | Confirmed (energy date); capacity date still "verify" — sources differ |
| FERC 2222: MISO Phase 1 by 1 Jun 2027, Phase 2 by 1 Jun 2029 | Tracker digest, July 2026 | 2026-11-06 | Confirmed — no change |
| FERC 2222: CAISO fully implemented 1 Nov 2024; SPP Q2 2030 + final approval 21 May 2026 | Cited source only (tracker PDF lines beyond 271 unread) | 2026-11-07 | Open — not independently re-verified |
| IEEE 1547 series validity/expiry/revision status (§3) | https://sagroups.ieee.org/scc21/ieee-1547-series/ (crawled 2026-11-06 19:39 UTC, "as of April 2026") | 2026-11-06 | Confirmed exactly — no change |
| SBOM tool versions: Syft 1.56.0 / Grype 0.121.0 / Trivy 0.76.1 / OSV-Scanner 2.7.0 / cdxgen 13.5.0 | Tools' own release pages (see §4) | 2026-11-07 | Updated — §4 table corrected; ORT still unpinned |
| Trivy supply-chain compromise detail (CVE-2026-33634, deleted releases, v0.70.0 first clean) | Trivy security advisory; CISA KEV (26 Mar 2026) | 2026-11-07 | Updated — §4 note added |
| CycloneDX 1.7 (late 2025, ECMA-424 2nd Ed. Dec 2025); SPDX 3.1 RC (Jan 2026); EU CRA full obligations from 11 Dec 2027; Art. 14 duties from 11 Sep 2026 | BSI TR-03183-2; SPDX/CycloneDX release notes; TuxCare / SOTA (9 Sep 2026) / Vanta | 2026-11-07 | Updated — §4 corrected (was "December 2027") |
| OpenADR 3.1 certification profiles | Web search (spec 3.1.0 exists; no certification-profile availability found) | 2026-11-07 | Open — still not verified; §6 wording stands |
| Spot-checks across other phases (not phase-10 files; corrections recorded in `ledger/_iterations/iteration-09/findings.md`, not applied here): Wasmtime CVEs (P7 — flaw-range lumping + missing GHSA-m9f7-9w4p-gv38), NATS CVE-2026-33222 fix-version wording + missing CVE-2026-33223 (P1/P2) | Wasmtime/NATS release and advisory pages | 2026-11-06/07 | Corrections needed elsewhere — see iteration-09 findings |
| Spot-checks confirmed with no change: Rust 1.100 planned 12 Nov 2026 (stable 1.99.1 on 15 Oct 2026); pandapower 3.5.6; HiGHS/highspy 1.15.1; Clarabel.rs 0.11.1; OpenEMS 2026.11.0; PyTorch 2.15 vs PyG 2.8.0 skew (still open, already flagged in P8) | PyPI / release pages (see iteration-09 findings for URLs) | 2026-11-06/07 | Confirmed — no change |
