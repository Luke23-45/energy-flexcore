# Phase 6 — Edge and Protocol Integration: Research

| Field | Value |
|---|---|
| Phase | 6 — Stage 6 Edge and protocol integration |
| Checked | 7 November 2026 |
| Method | `browser.search` + `browser.open` on primary sources only (official docs, repos, standards bodies). Secondary sources are marked as such and are not used as the basis for a version claim. |
| Rule | Do not invent versions. Anything not verified here is labelled **ASSUMPTION** or listed as an open verification task (§8). |

## 1. OpenEMS

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Latest release | **2026.11.0**, published 31 October 2026 (monthly release train: 2026.10.0 on 1 Oct 2026, 2026.9.0 before it). | Zenodo record for OpenEMS/openems 2026.11.0 — https://zenodo.org/records/23834676 |
| Edge architecture | Edge and Backend are Java (JRE required), OSGi component model. Edge runs an input–process–output (IPO) cycle; a Cycle component sets the process image, then executes Controllers in Scheduler order. Physical hardware is abstracted by Natures (Java interfaces in `.api` OSGi bundles, e.g. `io.openems.edge.ess.api` with `SymmetricEss`, `ManagedSymmetricEss`); a Channel is a single piece of information about a component; EdgeConfig is the JSON definition of the Edge's configuration. | OpenEMS Edge architecture — https://openems.github.io/openems.io/openems/latest/edge/architecture.html |
| Edge ↔ Backend | Edge connects to Backend via `Controller.Api.Backend` over WebSocket, authenticating by **API key**; the Edge-facing endpoint is now the separate **Backend.Edge.App**, which relays to the central Backend's Edge.Manager and caches the API-key → Edge-ID mapping from the Metadata service. The Backend.Edge.App ↔ Edge.Manager protocol is designed with a **trusted-network assumption, without its own peer authentication**; the Edge Manager WebSocket port (default 8083) and Felix Web Console (default 8079) must be reachable only from trusted hosts. | OpenEMS Backend architecture — https://openems.github.io/openems.io/openems/latest/backend/architecture.html |
| Getting Started defaults | The official Getting Started guide configures `Controller.Api.Backend` with an API key and a `ws://` (unencrypted) URI in the lab example, uses `Metadata.Dummy` (accepts any password), and logs in to the Felix console as `admin`/`admin`. The guide states the setup is for testing only and not recommended for production. | OpenEMS Getting Started — https://openems.github.io/openems.io/openems/latest/gettingstarted.html |
| 2026.11.0 notes relevant to integration | Legacy Energy Scheduler v1 removed; ENTSO-E tariff components migrated to the new spot-tariff system; "Syncable" — data can now be transferred from Backend to Edge via subscriptions; Edge power control respects the inverter's maximum apparent power; IPv4 preferred in the network stack. | Zenodo 2026.11.0 record (above) |

**Implications for Phase 6:** pin OpenEMS 2026.11.0 (or the version the pilot
actually runs) in P6.10/P6.11 and re-check the channel list on every monthly
release — component IDs and controllers move. The Getting Started defaults
must never be copied into a pilot deployment. See contradiction note R6-C1.

## 2. OpenADR 3.0

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Normative reference | The **OpenADR 3 OpenAPI YAML (SwaggerDoc) Specification is the normative reference** and supersedes statements in other documentation. OADR 3 also comprises Definitions (information models, enumerations, security) and a User Guide; the reference implementation is members-only. | OpenADR Alliance, "OpenADR 3 Introduction and Certification Program" — https://openadr.memberclicks.net/openadr-3-0 |
| Relationship to 2.0 | OADR 3 is **not intended to replace OpenADR 2.0a/b**; it is an additional, simplified way to add OpenADR functionality. **3.0 is not backward compatible with 2.0** (REST/JSON + token auth vs XML/SOAP + TLS certificates). | Same source; OpenADR Alliance July 2025 webinar (secondary, corroborating) — https://www.OpenADR.org/assets/OADR3%20webinar%20PGE_Olivine_20250715.pdf |
| Certification | Only products/systems that went through the OpenADR Alliance Certification Program can claim OpenADR compliance or certified status. OADR 3 has several certification profiles per application scenario. Final verification must be conducted at an Alliance-appointed test house; an online test tool exists (https://test-tool.openadr.org, members) and certification requires a license (US $1,250 per the Alliance's certification page, including 3 hours with a Certification Manager). | https://openadr.memberclicks.net/openadr-3-0 ; https://openadr.memberclicks.net/openadr-3-certification |
| Versions | 3.0 released 2024; certification available for 3.0 and 3.0.1. | July 2025 Alliance webinar (secondary) |

**Open verification task:** the existence/scope of **OpenADR 3.1 (said to be
2025)** appears only in the July 2025 Alliance webinar slides, a secondary
source. Do not record 3.1 as established until the OpenAPI YAML package or
an Alliance publication is checked directly (P6.12/P6.19). **ASSUMPTION:**
none made about 3.1 in this ledger.

## 3. OCPP 2.0.1 / 2.1

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Versions | OCPP 2.0.1 released 2020 (approved as IEC 63584:2024); **OCPP 2.1 released January 2025** (final 23 January 2025 per secondary reporting); 2.1 is backward compatible with 2.0.1 and adds bidirectional power transfer with ISO 15118-20 (V2X), a DER control functional block, battery swapping, and new payment options. 2.0.1 is **not** backward compatible with 1.6. | Report §27 [R19]; corroborated by multiple secondary summaries of the Open Charge Alliance announcement. The OCA's own protocol pages were not directly opened in this check — see open verification task below. |
| Editions | Secondary sources report an **Edition 2** of OCPP 2.1 (and 2.0.1) published in 2026, with parts 5 and 6 carrying certification profiles/test cases and an April 2026 errata; one secondary source dates Edition 2 to 8 June 2026, another to December 2025. The dates conflict. | Secondary only — see open verification task. |

**Open verification task:** confirm against the Open Charge Alliance
(https://openchargealliance.org/protocols/open-charge-point-protocol/) the
exact 2.1 Edition 2 publication date, the parts 5/6 certification-profile
contents, and the errata — before D5 selects OCPP or P6.19 writes the
conformance plan. Do not cite a single Edition 2 date as fact in this
ledger. **ASSUMPTION:** none made about Edition 2 dates.

## 4. IEEE 2030.5 / CSIP

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Current revision | **IEEE 2030.5-2023** is the current (active) revision — "Revision Standard – Active". It maintains backward compatibility with IEEE 2030.5-2018 except for elimination of the requirements for mandatory DERControl modes, and adds function sets including EV charging. | IEEE 2030.5-2023 listing — https://www.en-standard.eu/ieee-2030-5-2023-ieee-standard-for-smart-energy-profile-application-protocol/ ; IEEE Standards Association — https://sagroups.ieee.org/2030-5/ (report §27 [R27]) |
| Architecture | RESTful application protocol over TCP/IP + HTTP, XML payloads, TLS, mandatory PKI (three-tier: root/manufacturer/device CAs per implementation guides). DER clients poll the server; function sets cover DER control, metering, demand response, pricing. | Same sources; implementation-guide summaries (secondary). |
| CSIP | The Common Smart Inverter Profile is the SunSpec Alliance test profile selecting the IEEE 2030.5 function sets required for grid-interconnection compliance (California Rule 21 default). SunSpec publishes the CSIP implementation guide and conformance test procedure; certification is via SunSpec Authorized Test Laboratories. | SunSpec Alliance certification pages — https://sunspec.org/certifications/ |

**Implication:** IEEE 2030.5/CSIP is a **later adapter** per §2.3, not a
Phase 6 deliverable. The research above is recorded so the D5 decision in
P6.01 is informed and so Phase 6 does not accidentally scope it in.

## 5. SunSpec Modbus

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| What it is | Vendor-neutral **device information models** (not a WAN protocol): numbered model blocks — 1 Common, 101–103 inverters, 120–126 grid-support functions, **701–712 DER**, 800-series storage, 200-series meters — encoded as Modbus registers after a `SunS` marker, or as JSON. IEEE 1547-2018 names SunSpec Modbus and IEEE 2030.5 as the two interoperable DER communication methods for the same functions. | SunSpec Alliance — https://sunspec.org/sunspec-modbus/ ; https://sunspec.org/specifications/ |
| Specification package | SunSpec Modbus Specification Package includes the DER Information Model Specification, the Information Model Reference (incl. approved 700-series DER models), model definitions (SMDX/JSON, on GitHub), the Energy Storage Model Description, and the SunSpec Modbus IEEE 1547-2018 Specification, Profile and Implementation Guide. | https://sunspec.org/sunspec-modbus/ |
| Certification | SunSpec Certified program via Authorized Test Laboratories; SunSpec Modbus for IEEE 1547 certification and a SunSpec Express Test service exist. | https://sunspec.org/certifications/ |

**Implication:** a SunSpec adapter's mapping tables (P6.12) must be built
from the model definitions (SMDX/JSON), not from a vendor's register
spreadsheet; scale factors are part of the model and must be honoured.

## 6. NIST SP 800-82 Rev. 3 / Rev. 4

| Item | Finding (checked 2026-11-07) | Source |
|---|---|---|
| Rev. 3 (current final) | **SP 800-82 Rev. 3, Guide to Operational Technology (OT) Security, published September 2023** — the current final baseline. | NIST CSRC; report §17.1 / §27 [R26] |
| Rev. 4 (draft) | **Initial public draft published 21 September 2026; comments due 30 November 2026.** It realigns the guide with CSF 2.0 (Govern function), widens sector coverage, and adds governance/asset-management/monitoring/cloud-IIoT/zero-trust discussion; a section on post-quantum cryptography is included. Appendices C/D are planned to become web resources; appendices E/F separate documents. **Rev. 4 is a draft, not a final standard; Rev. 3 remains authoritative until Rev. 4 is finalized.** | NIST CSRC — https://csrc.nist.gov/pubs/sp/800/82/r4/ipd |

**Implication for P6.03:** the D6 review cites Rev. 3 as the governing
reference and may note Rev. 4 draft direction, but must not cite Rev. 4 as
if it were final or as a compliance requirement.

## 7. Contradictions and traps found (recorded honestly)

| ID | Finding |
|---|---|
| **R6-C1 — OpenEMS native auth ≠ FlexCore mTLS** | The report (§3.3, §17.3, §18.1) requires FlexCore's edge↔central channel to be outbound mTLS. OpenEMS's own Edge↔Backend authentication is an **API key over WebSocket** (Backend.Edge.App caches the API-key → Edge-ID mapping), and the Backend.Edge.App ↔ Edge.Manager link is explicitly trusted-network without peer authentication. These are two different channels. FlexCore must not claim OpenEMS speaks mTLS natively, must not downgrade its own channel to API-key-only auth, and must treat the OpenEMS API-key exchange as adapter-internal. The report does not itself conflate them, but an implementer reading §2.3 ("OpenEMS integration") together with §3.3 could. P6.07 guards this. |
| **R6-C2 — OpenEMS Getting Started defaults are unsafe as a deployment model** | The official guide's lab setup (`admin`/`admin`, `Metadata.Dummy` accepting any password, `ws://`) is labelled testing-only in the guide itself. Any Phase 6 lab built by following it verbatim and then described as a pilot configuration would be a security defect, not a shortcut. P6.06/P6.11 guard this. |
| **R6-C3 — Report's OCPP wording vs Edition 2** | Report §2.3 says "start with 2.0.1 or 2.1 according to target fleet and vendor support". Since the report's cut-off, Edition 2 material and an April 2026 errata have appeared (secondary sources, dates conflicting). D5 must name the **edition** as well as the version, or the conformance target is ambiguous. |
| **R6-C4 — No contradiction found in the §23 Stage 6 exit gate** | The exit gate ("read-only end-to-end site integration works in a lab; command path remains disabled unless explicitly approved for a test environment") is consistent with §2.1 v0.4.0 ("Read-only or test-lab control by default") and §21.4. No correction to the report is proposed on this point. |

## 8. Open verification tasks (not facts)

1. OpenADR 3.1 existence/scope — check the Alliance's OpenAPI YAML
   package or an Alliance publication directly.
2. OCPP 2.1 Edition 2 date, parts 5/6 contents, April 2026 errata — check
   the Open Charge Alliance directly.
3. The selected protocol's conformance-suite access terms (membership,
   license, fee) — confirm in P6.19 before writing tests; the OpenADR
   US $1,250 certification-license figure above is from the Alliance's own
   certification page and should still be re-confirmed at procurement time.
4. The pilot partner's actual OpenEMS version in the lab (may differ from
   2026.11.0) — record in P6.10.
5. Whether the pilot site's gateway hardware supports hardware-backed key
   storage (§18.3) — confirm in P6.06; if not, record the fallback and its
   risk.
