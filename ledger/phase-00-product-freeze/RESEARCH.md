# RESEARCH — Phase 0: Product and Domain Freeze

Research cut-off for this ledger: **7 November 2026** (same as report §27).
Checked: **7 November 2026**. All sources below are primary sources only
(the standard body, the regulator, or the vendor's own publication), except
where a source is explicitly labelled secondary. Versions and statuses are
recorded as found on the check date; anything that could not be verified is
labelled **ASSUMPTION** or written as an open verification task, not a fact.

## 1. FERC Order 2222 — status

| Item | Finding | Source |
|---|---|---|
| Order issued | Order No. 2222 issued 17 September 2020 (Docket No. RM18-9-000); Orders 2222-A (18 March 2021) and 2222-B (17 June 2021) on rehearing | FERC Order 2222 Explainer, https://www.ferc.gov/ferc-order-no-2222-explainer-facilitating-participation-electricity-markets-distributed-energy |
| What it does | Requires each RTO/ISO to revise tariffs so DER aggregations can participate directly in capacity, energy, and ancillary-service markets; aggregators become a market-participant type; minimum aggregation size requirement must not exceed 100 kW | FERC Order 2222 Fact Sheet, https://www.ferc.gov/media/ferc-order-no-2222-fact-sheet |
| Not a protocol | The order is a regulatory framework mediated through individual RTO/ISO tariffs. It does not create a universal market API. (Matches report §2.3, §22.1 ADR-011.) | FERC Fact Sheet (above); report [R1] |
| ERCOT | Order 2222 does not apply in ERCOT (outside FERC jurisdiction); ERCOT has its own DER-integration pilot programme | FERC Explainer (above) |
| ISO-NE effective date | **Effective 1 November 2026** — participants can begin to register DER aggregations (DERAs). Minimum aggregation 100 kW; a single resource or aggregation ≥ 5 MW at a single node must participate standalone or through a more geographically limited aggregation. Seven participation models. ISO-NE readiness page last updated **4 November 2026**. Sandbox opened 30 July 2026. | ISO New England, https://www.iso-ne.com/participate/support/participant-readiness-outlook/ferc-order-no-2222 |
| CAISO | Aggregation programme operating since 1 November 2024 (first operator to complete implementation) | Secondary: field digest 21 September 2026, https://ebungo.orem.in/field-digest-ferc2222-2026-09-21.html — **verify against a CAISO primary source before relying on this date** |
| NYISO | Targeted full compliance by end of 2026; requested an extension on 15 April 2026 to 30 April 2027; FERC accepted the extension on 30 July 2026 | Secondary: FERC Order 2222 & DER Policy and Implementation Report, July 2026 (PDF), https://cuswebsite.blob.core.windows.net/2222tracker/Tracker-Report-July-2026.pdf — **verify against the FERC docket before relying on the extension date** |
| PJM / MISO / SPP | PJM full model 1 February 2028; MISO aggregated-resource enrollment 1 June 2029, day-ahead/real-time participation 1 January 2030; SPP Q2 2030 | Secondary: same July 2026 tracker PDF and callmepower summary — **open verification task: confirm each date against the relevant ISO/RTO filing before any D1 decision cites it** |

**Implication for Phase 0:** the report's statement (§2.3) that implementation
is per-ISO and staggered is confirmed and now has a live data point (ISO-NE,
1 November 2026). Nothing in this changes the freeze: D1 still decides
jurisdiction; no market adapter is built on a generic Order 2222 mapping.

## 2. IEEE 1547-2018 — status

| Item | Finding | Source |
|---|---|---|
| Standard | IEEE Std 1547-2018, *IEEE Standard for Interconnection and Interoperability of Distributed Energy Resources with Associated Electric Power Systems Interfaces* | IEEE Standards Association, https://standards.ieee.org/ieee/1547/5915/ (report [R18]) |
| Status | **Revision Standard — Active.** Approved 15 February 2018; published 6 April 2018; revision of IEEE Std 1547-2003 | ANSI webstore listing, https://webstore.ansi.org/SDO/IEEE (IEEE 1547-2018 entry); standard front matter (ISBN 978-1-5044-4639-6, STD22967) |
| Scope | Technical specifications for, and testing of, interconnection and interoperability between utility electric power systems (EPSs) and DERs; requirements relevant to performance, operation, testing, safety, maintenance; applicable to all DER technologies at typical primary/secondary distribution voltages; written considering the DER as a 60 Hz source | ANSI webstore / standard abstract (above) |
| Clause 5 | Includes reactive-power and voltage/power-control capabilities; exact capability and settings depend on the applicable category, performance requirements, utility profile, and equipment (matches report §11.3) | Report §11.3, citing [R18]; NLR clause-by-clause summary, https://www.nlr.gov/grid/ieee-standard-1547/clause |
| Conformance testing | Equipment safety/protection and conformance testing under IEEE 1547.1 remain separate responsibilities; implementing an adapter is not a conformance claim | Report §11.3, §2.3 |
| Revision in progress | A current revision, IEEE P1547, is underway (noted in a September 2024 Sandia presentation); **1547-2018 remains the Active standard as of this check** — do not cite a successor as current | Sandia, https://www.sandia.gov/app/uploads/sites/256/2024/09/IEEE-1547-for-2024-LDES-Workshop-final.pdf |
| Companion guides | IEEE 1547.9-2022, *Guide for Using IEEE Std 1547 for Interconnection of Energy Storage DERs*, exists; the 1547 family includes testing (1547.1) and application guides | NREL DER integration webinar, July 2025, https://www.nlr.gov/docs/fy25osti/92436.pdf |

**Open verification task (updated 7 November 2026, iteration-02 review):**
the IEEE SA project page (https://standards.ieee.org/ieee/1547a/7696/, checked
7 November 2026) lists P1547 under "Current projects that have been authorized
by the IEEE SA Standards Board to develop a standard", and lists 1547-2018
under "Standards approved by the IEEE SA Standards Board that are within the
10-year lifecycle" — so the revision is an authorized active project and
1547-2018 remains Active. No expected publication date for the revision was
found on that page; do not assert one. The task is closed on status and remains
open only on the publication date, before any Phase 0 artefact cites a
successor to 1547-2018.

## 3. NIST SP 800-82 — OT security reference

| Item | Finding | Source |
|---|---|---|
| Current final version | SP 800-82 Rev. 3, *Guide to Operational Technology (OT) Security*, final, published 28 September 2023 | NIST CSRC, https://csrc.nist.gov/pubs/sp/800/82/r3/final (report [R26]) |
| Rev. 4 | **Initial Public Draft** released **21 September 2026**; restructured around NIST Cybersecurity Framework 2.0 (Govern function); expanded sector coverage (building automation, water/wastewater, food/agriculture, freight rail, maritime, IIoT/cloud convergence); adds Zero Trust principles (§5.3) and enterprise-risk-management alignment | NIST CSRC news, https://csrc.nist.gov/News/2026/draft-guide-to-ot-security ; draft PDF, https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-82r4.ipd.pdf (DOI 10.6028/NIST.SP.800-82r4.ipd) |
| Comment period | **Open through 30 November 2026** (after this ledger's 7 November 2026 cut-off) | NIST CSRC news (above) |
| Status rule for Phase 0 | Rev. 3 remains the current final reference; Rev. 4 is a draft and must not be cited as a final standard (matches report §17.1) | Report §17.1 |

**Implication for Phase 0:** P0.17 must cite Rev. 3 as final and Rev. 4 as a
draft with the comment deadline. Re-check Rev. 4 status at the next phase gate
(after 30 November 2026 the draft may change or finalise).

## 4. WASI / Component Model status (plugin model, P0.19)

| Item | Finding | Source |
|---|---|---|
| Baseline for first plugin SDK | WASI 0.2 / Component Model baseline (report §5.4) | Report §5.4, citing [R3] |
| WASI 0.3 | Rust `wasm32-wasip3` target documented as requiring nightly before the planned Rust 1.100 release on **12 November 2026**; at the 7 November 2026 cut-off, do not make WASI 0.3 a release blocker; re-evaluate after stable toolchain and host compatibility mature | Report §5.4, citing [R3]: https://github.com/WebAssembly/WASI/blob/main/docs/ComponentModelFeatures.md ; https://component-model.bytecodealliance.org/language-support/creating-runnable-components/rust.html |
| Wasmtime | Component Model embedding APIs and sandboxing mechanisms exist, but sandboxing must still be combined with host capability policy, resource limits, signatures, and isolation | Report §5.4, citing [R4]: https://docs.wasmtime.dev/security.html |

**Open verification task (Phase 0, P0.25):** on or after 12 November 2026,
confirm from the Rust release notes whether Rust 1.100 stable shipped and
whether `wasm32-wasip3` is stable; record the outcome. As of 7 November 2026
the stable toolchain is Rust **1.99.1** (see Phase 1 `RESEARCH.md`).

## 5. Cut-off check (P0.25 evidence)

| Claim in Phase 0 | Source | Changed after 7 Nov 2026 cut-off? |
|---|---|---|
| Order 2222 is a framework, not a protocol | FERC Fact Sheet | No — stable since 2020; ISO-NE effectiveness (1 Nov 2026) precedes the cut-off |
| IEEE 1547-2018 is Active | IEEE SA / ANSI | No change found as of 7 Nov 2026 |
| NIST SP 800-82 Rev. 3 final; Rev. 4 draft | NIST CSRC | Rev. 4 comment period runs to 30 Nov 2026 — status unchanged (still a draft) as of 7 Nov 2026 |
| WASI 0.3 not a release blocker | WASI Component Model feature status | Rust 1.100 planned 12 Nov 2026 — after the cut-off; not yet stable on 7 Nov 2026 |

No Phase 0 freeze decision is invalidated by a post-cut-off change found in
this check. The two after-cut-off events to re-check at the next gate are the
NIST Rev. 4 comment close (30 November 2026) and the Rust 1.100 stable release
(12 November 2026).

## 6. Sources NOT used / not verified

- No blog summary was used as a source for what a standard says (per
  `.agent/WRITING.md`). Secondary sources above (CAISO date, NYISO extension,
  PJM/MISO/SPP dates) are labelled secondary and listed as open verification
  tasks; no Phase 0 decision relies on them.
- **ASSUMPTION:** none in this file. Every dated claim above names its source.
  Where a date could not be confirmed from a primary source, the entry says so
  instead of asserting the date.
