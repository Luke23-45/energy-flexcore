# P0.17 — Threat model and security zones freeze

| Field | Value |
|---|---|
| Subtask | P0.17 — Freeze the threat model and security zones |
| Report trace | §17.1–§17.5 (security model, zones, required controls, threats, safety invariants), §3.3 (deployment topology), §18.2 (edge trust boundary) |
| Source | `docs/plan/Neryva_FlexCore_Product_and_Architecture_Report_v1.1.md` (product baseline; foundations specification governs on conflict per AGENTS.md §2) |
| External reference status | Per `ledger/phase-00-product-freeze/RESEARCH.md` §3 and report §17.1, [R26] |
| Inputs | P0.01 (scope/non-goals), P0.16 (tenancy/storage principles); NIST SP 800-82 research in `RESEARCH.md` |
| Status | Draft for Stage 0 review |
| Scope limit | These are **system requirements, not a certification claim** (§17.5, closing sentence). FlexCore is not certified and this document is not a safety case (AGENTS.md §3 rule 2). |

---

## 1. Purpose

Freeze the §17 threat model, security zones, required controls, and safety invariants as the security baseline for all later phases (ledger P0.17). Phase 1 security tests are derived from the §17.4 table and the §17.5 invariants.

## 2. Security-model reference (§17.1)

FlexCore interacts with operational technology. Security must reflect OT availability, reliability, and safety requirements, not just conventional web-application security (§17.1).

Reference status at the 7 November 2026 research cut-off, as recorded in `RESEARCH.md` §3:

| Item | Status |
|---|---|
| NIST SP 800-82 Rev. 3, *Guide to Operational Technology (OT) Security* | **Current final reference** — final, published 28 September 2023 (report §17.1, [R26]) |
| NIST SP 800-82 Rev. 4 | **Initial public draft only** — released 21 September 2026, comments due 30 November 2026; restructured around NIST Cybersecurity Framework 2.0 (Govern function), with expanded sector coverage and Zero Trust principles (`RESEARCH.md` §3) |
| Status rule | Rev. 4 is treated as a draft, never cited as a final standard; Rev. 3 remains the current final reference (report §17.1; `RESEARCH.md` §3) |

Re-check Rev. 4 status at the next phase gate: after 30 November 2026 the draft may change or finalise (`RESEARCH.md` §3, §5).

## 3. Security zones (§17.2)

Five zones, exactly as listed in §17.2:

| # | Zone | Contents (§17.2) |
|---|---|---|
| 1 | Enterprise IT zone | Browser access, identity, administrative workflows |
| 2 | Central application zone | Control plane, database, event broker, plugin host, compute workers |
| 3 | Customer integration/DMZ zone | Edge agent and approved gateways |
| 4 | OT/site control zone | Site EMS, SCADA, PLCs, local controllers |
| 5 | Field/device zone | Inverters, batteries, EVSE, meters, relays, and physical equipment |

**Inter-zone flow rule (§17.2):** permit only documented flows between zones. No direct database, NATS, or arbitrary plugin network access from a field-device network.

**Deployment-topology reinforcement (§3.3):** the site connection is initiated outward from the customer edge to the central platform (or through a customer-controlled broker/gateway); the edge agent connects over an outbound mTLS channel carrying approved commands only. Avoid inbound internet access into OT networks; a customer security architecture may require a DMZ, unidirectional gateway, jump host, or other approved pattern. A zone diagram that permits cloud-to-device access contradicts this freeze (ledger P0.17 risk note).

## 4. Required controls (§17.3 — full list, none dropped)

| # | Control | Report wording (§17.3) |
|---|---|---|
| 1 | Identity federation | OIDC/OAuth2 integration with customer identity provider |
| 2 | Authorization | RBAC plus resource/site/asset-scoped authorization |
| 3 | Strong approval | MFA and step-up approval for high-impact administrative/control actions where required |
| 4 | Transport identity | mTLS for edge and service identities |
| 5 | Credential hygiene | Short-lived credentials and certificate rotation |
| 6 | Command integrity | Signed commands, sequence numbers, expirations, and replay protection |
| 7 | Plugin integrity | Plugin signature/provenance and SBOM checks |
| 8 | Sandbox discipline | WASM sandbox limits and explicit capabilities |
| 9 | Network discipline | Network segmentation and allowlisted communication paths |
| 10 | Confidentiality | Encrypted transport and storage |
| 11 | Secret handling | Secrets manager integration; no secrets inside plugin config files or source control |
| 12 | Traceability | Audit logging for all control-plane and dispatch actions |
| 13 | Edge platform integrity | Secure boot/update policy for edge hosts where customer platform supports it |
| 14 | Continuity | Backup/restore tests and disaster recovery |
| 15 | Maintenance | Vulnerability management and dependency scanning |
| 16 | Response | Incident response and customer notification process |

## 5. Threat model (§17.4 — all 10 threats)

| # | Threat | Control (§17.4) |
|---|---|---|
| 1 | Forged/replayed command | mTLS, signature, nonce/sequence, expiry, idempotency and authorization evidence |
| 2 | Compromised plugin | WASM isolation, capability broker, resource limits, signature, no ambient network/filesystem |
| 3 | Malicious or incorrect telemetry | Source identity, validation, quality flags, residual checks, stale-data gates, model uncertainty |
| 4 | Cross-tenant leakage | Tenant context, RLS, scoped service identity, access tests, audit |
| 5 | OT lateral movement | Zone/DMZ boundaries, outbound-only edge connection, allowlisted paths, no direct cloud-to-device access |
| 6 | Broker duplication/replay | Idempotent consumers, unique IDs, sequence checks, outbox/inbox pattern |
| 7 | Unauthorized operator action | RBAC, step-up approval, separation of duties, immutable audit evidence |
| 8 | Plugin supply-chain compromise | Signed artifacts, provenance, SBOM, pinned dependencies, staged rollout |
| 9 | Clock drift | Time-quality monitoring, bounded command expiry, synchronization checks |
| 10 | Central outage | Site-local fallback and autonomous local EMS; no dependence on cloud for protection |

## 6. Safety invariants (§17.5 — all 10, protected platform code)

The following must be enforced by protected platform code and cannot be bypassed by plugins (§17.5). They are platform invariants, not plugin-configurable options (ledger P0.17 risk note).

| # | Invariant |
|---|---|
| 1 | No command without a valid asset/site identity and current command context. |
| 2 | No command without capability validation. |
| 3 | No command without required network feasibility evidence for the declared operating profile. |
| 4 | No command without authorization. |
| 5 | No stale or expired command. |
| 6 | No unapproved protocol adapter or target. |
| 7 | No direct physical command from an optimizer, GNN, MARL, LLM, or general plugin. |
| 8 | No cross-tenant command or telemetry access. |
| 9 | A missing/uncertain/stale model is not interpreted as a passing feasibility check. |
| 10 | Site-local safety and protection remain independent of FlexCore. |

Invariant 3 corresponds to the §12.5 rule that a failed or stale/incomplete model must not let the system infer feasibility; invariant 7 corresponds to §22.1 ADR-010.

## 7. Edge trust boundary (§18.2)

An edge agent is not trusted merely because it belongs to a customer site: it must authenticate each central session and validate every command. Conversely, the central platform must not assume that an issued command was applied until it receives device/site acknowledgement and telemetry evidence (§18.2). Acknowledgement is not physical delivery — the two are distinct gates in the command lifecycle (§14; P0.11).

## 8. What this document does not do

- It states **system requirements, not a certification claim** (§17.5): nothing here asserts that FlexCore, or any deployment of it, is certified, accredited, or a safety case.
- It makes no protection-grade claim: site-local safety and protection remain with local certified controllers (§17.5 item 10; §14.5; §22.1 ADR-012).
- It changes no §17.3 control, §17.4 row, or §17.5 item; the lists are reproduced, not edited.
- It selects no product: controls are requirements on later phases, not an implementation.

## 9. Acceptance-criteria coverage (ledger P0.17)

| Ledger acceptance criterion | Where satisfied |
|---|---|
| Rev. 3 final / Rev. 4 draft status stated correctly with dates (28 Sep 2023 final; 21 Sep 2026 draft, comments due 30 Nov 2026) | §2 |
| All five zones and the inter-zone flow rule stated | §3 |
| All §17.3 controls listed; none dropped | §4 (16 controls) |
| All 10 §17.4 threats and all 10 §17.5 invariants reproduced | §5, §6 |
| The "not a certification claim" qualifier present | Header (Scope limit), §8 |

Additional ledger steps covered: §3.3 outbound-only topology reinforcement (§3), §18.2 edge trust boundary (§7), scope-limit statement (header, §8). Verification method (ledger): comparison against §17.1–§17.5; NIST status cross-checked against `RESEARCH.md` §3.
