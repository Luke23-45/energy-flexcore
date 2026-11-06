# Phase 9 — Customer Pilot Qualification: Research

**Checked:** 7 November 2026 (the report's research cut-off, §27)
**Rule:** Primary sources only (official docs, repos, standards bodies, regulators). Where a
primary source could not be verified, the item is recorded as an open verification task, not
as a fact. Versions and statuses below are as found on the check date; re-verify at the time
of use — standards, drafts, and implementation dates move.

## 1. FERC Order 2222 — implementation status

**What it is:** US FERC Order No. 2222 (issued 17 September 2020, docket RM18-9-000) requires
RTOs/ISOs to revise tariffs so DER aggregations (100 kW minimum) can participate in wholesale
energy, capacity, and ancillary-services markets. It is a regulatory framework, not a protocol
(report §2.3, [R1]).

**Status as checked 2026-11-07:**

| RTO/ISO | Status |
|---|---|
| CAISO | Fully implemented; effective 1 November 2024 — the only RTO/ISO fully live as of early/mid-2026 |
| ISO New England | Energy and ancillary-services participation targeted/effective 1 November 2026 |
| NYISO | Full implementation date extended to 30 April 2027 (FERC accepted the extension 30 July 2026; original date was 31 December 2026); partial implementation in the interim |
| PJM | DER aggregation participation model expected effective 1 February 2028 (energy); capacity-market access reported for the 2028/2029 delivery year — **verify the capacity date at time of use, sources differ** (see below) |
| MISO | Two-phase implementation: Phase 1 by 1 June 2027, Phase 2 by 1 June 2029 |
| SPP | Proposed second quarter of 2030 — the latest schedule |
| All six RTOs/ISOs | Compliance filings approved by FERC (SPP received its final compliance approval by letter order 21 May 2026, addressing the September 2025 compliance order) |

**Implication for this phase (P9.11):** a pilot justified on wholesale aggregation revenue is
only credible in a region where implementation is live. In an SPP or MISO footprint the
market leg of the pilot is not yet available; the pilot scope (P9.01) must reflect that.
ERCOT is not under FERC jurisdiction and is not subject to Order 2222.

**Sources:**

- FERC Order 2222 Fact Sheet — https://ferc.gov/media/ferc-order-no-2222-fact-sheet
  (report [R1])
- FERC Order 2222 & DER Policy and Implementation Report, July 2026 (CUSLN / FERC2222.org
  tracker) — https://cuswebsite.blob.core.windows.net/2222tracker/Tracker-Report-July-2026.pdf
- PNNL, "FERC Order 2222 Considerations for RERRAs and EDCs", May 2026 —
  https://www.pnnl.gov/sites/default/files/media/file/FERC%20Order%202222%20Considerations%20for%20RERRAs%20and%20EDCs.pdf
- CRA International, "Grid under pressure", January 2026 (secondary analysis of the same
  filings) — https://www.crai.com/insights-events/publications/grid-under-pressure-flexibility-centered-large-load-policies-overlook-recent-lessons/

**Open verification task:** PJM capacity-market date — one secondary source reports capacity
participation from 1 February 2027; the July 2026 tracker emphasises the 1 February 2028
energy date. Confirm against PJM's tariff/filing before writing it into P9.11.

## 2. IEEE 1547-2018 — status and revision

**Status as checked 2026-11-07 (IEEE SCC21 1547 series table, as of April 2026):**

- **IEEE 1547-2018** (Standard for Interconnection and Interoperability of DER): valid;
  standard expires 2028; **revision in ballot**.
- **IEEE 1547a-2020** (Amendment 1, abnormal operating performance Category III flexibility):
  valid to 2032.
- **IEEE 1547.1-2020** (conformance test procedures): valid to 2030. **P1547.1a** (Amendment 1
  to the test procedures): in ballot.
- **IEEE 1547.2-2023** (application guide) and **IEEE 1547.3-2023** (guide for cybersecurity
  of DER interconnected with electric power systems): valid to 2033. The 1547.3 guide is
  directly relevant to P9.07 (security assessment) — read it, do not assume its contents.
- **IEEE 1547.9-2022** (guide to using 1547 for energy-storage interconnection): valid to 2032.
- **IEEE 1547.4-2011** (island systems) and **IEEE 1547.6-2011** (distribution secondary
  networks) and **IEEE 1547.7-2013** (distribution impact studies): in revision.
- **P1547.10** (recommended practice for DER gateway platforms): new project, in progress.

**Implication for this phase:** P9.12 must record the standard and profile *as locally
adopted* — adoption varies by jurisdiction (some regulators still reference superseded
editions; e.g. one US state division was still anchored to the 2003 edition in mid-2026).
FlexCore's own use of IEEE 1547-2018 Clause 5 capabilities depends on category, utility
profile, and equipment (report §11.3). The in-ballot revision means the 2018 edition's
requirements must be re-checked when the revision publishes; P9.27 carries that as a
watch item.

**Sources:**

- IEEE SCC21, IEEE 1547 Series table — https://sagroups.ieee.org/scc21/ieee-1547-series/
- IEEE Standards Association, IEEE 1547-2018 — https://standards.ieee.org/ieee/1547/5915/
  (report [R18])
- SIS (Swedish Institute for Standards) record for IEEE 1547-2018 (status: valid; corrected
  by errata; amended by 1547a-2020) —
  https://www.sis.se/en/produkter/energy-and-heat-transfer-engineering/power-stations-in-general/ieee-1547-2018/

## 3. NIST SP 800-82 — Rev. 3 baseline, Rev. 4 draft

**Status as checked 2026-11-07 (verified against the NIST CSRC primary page):**

- **SP 800-82 Rev. 3**, *Guide to Operational Technology (OT) Security*: final, published
  September 2023 (28 September 2023). **This is the baseline for P9.07.**
- **SP 800-82 Rev. 4**, *Guide to Operational Technology (OT) Security*: **initial public
  draft, published 21 September 2026; comment period open through 30 November 2026.**
  It is a draft, not a final standard, and its content may change before publication.
- Rev. 4 changes (draft): restructured around the NIST Cybersecurity Framework (CSF) 2.0,
  including reorganising the risk-management section around the CSF Govern Function;
  expanded OT sector coverage (building automation and control systems, water/wastewater,
  food and agriculture, freight rail, maritime vessels, IIoT and cloud convergence);
  expanded discussion of OT risk alignment with enterprise risk management (NIST IR 8286r1);
  a new appendix on adopting the NIST Risk Management Framework (Appendix F); expanded
  guidelines on asset management, network monitoring and detection, security architecture
  focused on protecting system-management functions, and zero trust principles.

**Implication for this phase:** P9.07 uses Rev. 3 as the baseline and records Rev. 4 deltas
as draft observations only. Because the comment period closes 30 November 2026 — after this
ledger's cut-off — the final Rev. 4 must be re-checked in Phase 10 (P10.16) and the P9.07
assessment updated if the final text changes anything material.

**Sources:**

- NIST CSRC, SP 800-82 Rev. 4 initial public draft —
  https://csrc.nist.gov/pubs/sp/800/82/r4/ipd (verified by direct fetch 2026-11-07)
- NIST CSRC, SP 800-82 Rev. 3 final — https://csrc.nist.gov/pubs/sp/800/82/r3/final
  (report [R26])

## 4. Hardware-in-the-loop testing practice

**What was verified:**

- **IEEE 1547.1-2020** defines the conformance test procedures for DER equipment (report
  §11.3, §19.1). Its procedures typically involve dozens to hundreds of measurement points
  per grid-support function, so automation is the norm for certification-scale testing.
- **Power HIL (PHIL)** is the established method for executing IEEE 1547.1 ride-through
  tests against real inverters: a real-time simulator drives a power amplifier that emulates
  the grid, with precise voltage/frequency/ROCOF disturbance injection and waveform capture.
  Documented example: RPI / Smarter Grid Solutions PHIL facility (OPAL-RT OP1400 amplifier)
  testing an SMA inverter for IEEE 1547.1-2020 modes (constant power factor, constant
  reactive power, volt-var, volt-watt, voltage/frequency ride-through, return to service).
- **SunSpec System Validation Platform (SVP)** — open-source automated test scripts for IEEE
  1547.1-2020 (and UL 1741 SA) ride-through and grid-support-function tests, driving OPAL-RT
  (RT-LAB Python API) and Typhoon HIL systems; drivers for SunSpec Modbus, IEEE 2030.5,
  IEEE 1815, Modbus. Repositories: https://github.com/jayatsandia/svp_1547.1 and
  https://github.com/jayatsandia/svp_energy_lab (Sandia National Laboratories / SIRFN).
- **HIL platforms in use for this class of work:** OPAL-RT (ePHASORSIM/HYPERSIM, RT-LAB),
  RTDS/RSCAD (FPGA-based real-time simulation), Typhoon HIL (power-electronics HIL).
  Controller HIL (CHIL) and power HIL (PHIL) are distinct: PHIL additionally needs a power
  amplifier and an interface algorithm (e.g. Ideal Transformer Method) with a stability
  condition.
- The validation pathway used in the literature is digital-twin/simulation → HIL/PHIL →
  field, with the same disturbance profiles and seeds at each stage so evidence is
  comparable.

**Implication for this phase (P9.15):** the HIL plan must state CHIL vs PHIL, name the
platform, and reuse the IEEE 1547.1-2020 procedures where they apply. HIL evidence
supports FlexCore's integration claim; it does not certify the equipment (conformance
testing under IEEE 1547.1 remains a separate responsibility, report §11.3).

**Sources:**

- OPAL-RT, "PHIL-Based IEEE 1547.1 Smart Inverter Testing" —
  https://www.opal-rt.com/power-in-mind/volume-8/article-3/
- Sandia / SIRFN, "Commercial PV Inverter IEEE 1547.1 Ride-Through Assessments Using an
  Automated PHIL Test Platform" (Energies 2021) —
  https://psecommunity.org/wp-content/plugins/wpor/includes/file/2303/LAPSE-2023.17925-1v1.pdf
- SunSpec SVP — https://sunspec.org/sunspecsvp (referenced in the Sandia material above)

**ASSUMPTION (labelled):** the specific HIL platform, lab availability, and cost for the
pilot are not established by these sources and are not stated here. P9.15 must select and
record them for the deployment.

## 5. OpenADR 3.0 — certification status

**Status as checked 2026-11-07:**

- OpenADR 3.0 is a separate, simplified API-based standard; it is **not backward compatible**
  with OpenADR 2.0, and it does not replace 2.0a/2.0b.
- **First OpenADR 3.0 certified products were announced 24 March 2025** (DISTRIBUTECH 2025):
  E.ON Energy Networks (first VTN solution certified), EVoke Systems, Universal Devices
  (VEN). Certification is through the OpenADR Alliance Certification Program at appointed
  test facilities (10 approved test facilities reported in the Alliance's 2025 material; the
  Alliance reported 319 certified products overall at that time).
- OpenADR 3.1 was referenced in Alliance material in 2025; its certification-profile status
  was not verified in this check.

**Implication for this phase (P9.04):** if the pilot uses OpenADR, record whether the
counterparty requires 2.0b or 3.x, and which certification profile applies. Only products
that went through the Alliance Certification Program may claim certified status (report
[R29]); FlexCore's adapter implementation alone is not a certification claim.

**Sources:**

- OpenADR Alliance, first OpenADR 3.0 certified products (24 March 2025) —
  https://openadr.memberclicks.net/index.php?option=com_content&view=article&id=233:first-openadr-3-0-certified-products&catid=21:press-releases&Itemid=121
- OpenADR Alliance, OpenADR 3.0 introduction and certification —
  https://www.openadr.org/openadr-3-0 (report [R29])
- OpenADR Alliance, OpenADR 3 introduction and certification program —
  https://openadr.memberclicks.net/openadr-3-0

**Open verification task:** OpenADR 3.1 certification-profile availability and the current
count of certified 3.x products — not verified in this check; confirm before naming 3.1 in
P9.04.

## 6. SBOM and license-scanning tools (background for the D4 handover to Phase 10)

Phase 9 does not close D4, but it must not introduce pilot dependencies that Phase 10 cannot
audit (P10.19). The following is background only, for the handover note in P9.30.

**Formats (verified against multiple 2026 sources):** SPDX (ISO/IEC 5962:2021, Linux
Foundation) and CycloneDX (OWASP) are the two machine-readable SBOM formats in practice.
Germany's BSI TR-03183-2 v2.1.0 (August 2025) requires CycloneDX 1.6+ or SPDX 3.0.1+,
SHA-512 hashes, and license identifiers as SPDX expressions. The EU Cyber Resilience Act
(Regulation (EU) 2024/2847) requires a machine-readable SBOM (Annex I, Part I, para. 2(c);
Article 13 due diligence for third-party components); vulnerability-reporting duties under
Article 14 began 11 September 2026, with the full SBOM mandate landing December 2027.

**Tools reported as current in mid-2026 sources (re-verify versions at time of use):**
Syft (Anchore, Apache-2.0) for SBOM generation (reported ≥1.30 in mid-2026 baseline stacks;
v1.46.0 cited in an October 2025 benchmark study); Grype (Anchore) for vulnerability
scanning on an SBOM (reported ≥0.100); OSV-Scanner (Google, OSV database, reported ≥2.2);
Trivy (Aqua) for combined SBOM + vulnerability + misconfiguration scanning (v0.72.0 cited
in the same benchmark study); cdxgen (OWASP) for CycloneDX generation; OSS Review Toolkit
(ORT) for comprehensive multi-ecosystem license scanning and policy evaluation. **Tool
versions move quickly; the versions above are as reported by the cited sources, not
verified by this check against the tools' own release pages.**

**Sources:**

- CRA Decoded, "CRA SBOM Tools: Syft, Grype, Snyk, Anchore (2026)" —
  https://cra-decoded.com/blog/posts/cra-toolchain-open-source-vs-commercial-2026/
- SOTA, "CRA SBOM Requirements 2026" —
  https://www.sota.io/blog/cra-sbom-software-bill-of-materials-eu-compliance-2026
- Augment Code, "SBOM for Agent-Driven Pipelines" —
  https://www.augmentcode.com/guides/sbom-for-agent-driven-pipelines

**Open verification task:** confirm the current Syft/Grype/Trivy/ORT release versions and
the SPDX/CycloneDX format versions against each tool's own repository at the time P10.19
runs; do not copy the versions above into a release record without that check.

## 7. Items NOT verified in this check

- OCPP 2.1 device/fleet support for the pilot's specific EVSE vendors (report [R19] states
  OCPP 2.1 was released in 2025 with expanded V2X/smart-charging capability; device support
  remains implementation-specific). Verify per vendor in P9.04.
- IEEE 2030.5 / CSIP regional profile and certification planning for the pilot region
  (report [R27]). Verify in P9.04/P9.12.
- Any pilot-customer-specific fact (network data, vendor firmware versions, contract terms).
  All such facts are established inside the subtasks, not here.
