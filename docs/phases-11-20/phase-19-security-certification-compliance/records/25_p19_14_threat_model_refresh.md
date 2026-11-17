<!-- P19.14 — Threat-model refresh. HONEST RECORD: a DESIGN record that
     EXTENDS the frozen P0.17 threat model to the surfaces that did not
     exist at Phase 0. The frozen file
     (ledger/phase-00-product-freeze/17_threat_model.md) is NEVER edited
     — it is byte-identical after this batch; the refresh is a new
     record citing it. STRIDE-per-surface pass recorded as design
     requirements, not test outcomes. Pen-test scope (P19.13) derives
     from these surfaces. No certification or security-posture claim is
     made. -->
# P19.14 — Threat-model refresh: STRIDE-per-surface extension of the frozen P0.17 model to the P13/P14/P16/P18 surfaces (design record; P0.17 never edited)

| Field | Value |
|---|---|
| Subtask | P19.14 — Threat-model refresh (`ledger_phase_19.md` §P19.14) |
| Wave / agent | Phase 19 wave 1, Agent B |
| Status | **DESIGN RECORD — WRITTEN.** Requirements-level analysis only; no test ran, no control was exercised, no threat was "mitigated in practice" — every control cited is a landed WRITTEN-UNRUN producer record whose status is carried, not upgraded. |
| Frozen baseline | `ledger/phase-00-product-freeze/17_threat_model.md` (P0.17): five zones (§3), 16 required controls (§4), 10 threats (§5), 10 safety invariants (§6), §18.2 edge trust boundary (§7). **This record edits none of it.** |
| Sources | P0.17 (frozen baseline, cited throughout); producer records per row in §3–§4 (all verified on disk 2026-11-17); [S-163] (IEC 62443-4-2 component-requirements framing — the control-citation style, via the P19.03 linkage planned as Agent A's concurrent record) |
| Not-a-certification | Per `ledger/phase-10-v1-release-hardening/25_security_consolidation.md` §7 (verbatim discipline, AGENTS.md §3 rules 2–3): FlexCore is not certified, not a safety case, and not permission for live grid control. This refresh states requirements and design analysis; it asserts no achieved security posture. |

## 1. Honest status — the plain statement

**The frozen P0.17 file is untouched.** P0.17 froze the §17 threat
model at Phase 0 — five zones, 16 controls, 10 threats, 10 invariants —
and covers none of the surfaces that phases 11–18 later introduced.
This record is the refresh-as-design the phase-19 plan orders (plan §2
P19.14): it extends threat coverage to four new surfaces, records a
STRIDE pass per surface, maps each surface into the frozen five-zone
model, and confirms none of the ten §17.5 invariants is relaxed. It is
a NEW record citing the frozen one — never an edit to it.

## 2. Method

1. **STRIDE per surface.** Each new surface is walked through the six
   STRIDE categories (Spoofing, Tampering, Repudiation, Information
   Disclosure, Denial of Service, Elevation of Privilege). **Methodology
   note (ASSUMPTION):** no STRIDE source exists in gap analysis §11, so
   STRIDE is used here as a named, standard threat taxonomy only — no
   source-specific claim attaches to its use, and each category's
   treatment below stands on the cited control record, not on the
   taxonomy.
2. **Citation discipline.** For each threat cell the "existing control"
   column cites the landed producer record by path and carries its
   status. Nothing is re-decided; nothing is claimed as tested.
3. **Zone mapping.** Each surface is placed in the P0.17 five-zone
   model (§5) — a *design placement*, extending the model on paper;
   P0.17 §3 remains frozen and unchanged.

## 3. The surface registry (new surfaces only — P0.17's own coverage is unchanged)

| # | Surface | Owning phase | Primary records (paths verified 2026-11-17) |
|---|---|---|---|
| SR-1 | Identity: OIDC federation, sessions, SCIM, machine identity, API keys, approval workflow | P13 | `docs/phases-11-20/phase-13-enterprise-identity-integration/records/01_p13_01_identity_architecture.md`, `.../02_p13_02_oidc_token_validation.md`, `.../03_p13_03_scim_provisioning.md`, `.../05_p13_05_role_model.md`, `.../07_p13_07_machine_identity.md`, `.../08_p13_08_api_keys_webhook_secret_format.md`, `.../06_p13_06_approval_workflow_engine.md`, `.../42_p13_16_session_management.md` (same directory) |
| SR-2 | Protocol adapters: aggregator server, per-protocol clients, adapter permission boundary, observability | P14 | `docs/phases-11-20/phase-14-protocol-device-connectivity/records/02_p14_02_aggregator_server.md`, `.../03_p14_03_security_design.md`, `.../64_p14_16_adapter_permission_model.md`, `.../84_p14_21_protocol_observability.md`, `.../83_p14_20_backpressure_rate_limiting.md` (same directory) |
| SR-3 | Market records: market-records store, settlement/telemetry pipelines, market grant model, jurisdiction register | P16 | `docs/phases-11-20/phase-16-market-integration-settlement/records/01_p16_01_market_records_store.md`, `.../22_p16_08_telemetry_export_pipelines.md`, `.../23_p16_09_settlement_ingestion.md`, `.../61_p16_19_market_adapter_permission_model.md`, `.../46_p16_18_market_jurisdiction_register.md` (same directory) |
| SR-4 | OTA / edge fleet: update-bundle format, attestation, device identity lifecycle, fleet registry, update command interlock | P18 | `docs/phases-11-20/phase-18-edge-fleet-lifecycle-ota/records/02_p18_02_update_bundle_format.md`, `.../26_p18_13_attestation_design.md`, `.../21_p18_08_identity_lifecycle.md`, `.../25_p18_12_fleet_registry_hardening.md`, `.../28_p18_15_update_command_interlock.md`, `.../07_p18_07_update_evidence_audit_pack.md` (same directory) |

## 4. STRIDE-per-surface pass (design requirements — not test outcomes)

Producer status is carried as in the P19.10 map: every cited control is
WRITTEN-UNRUN (or a design record / frozen baseline), and the residual
column is a requirements-level note, never a tested claim.

### 4.1 SR-1 — Identity surface (P13)

| STRIDE | Design threat on this surface | Existing control (cited) | Residual note |
|---|---|---|---|
| Spoofing | Forged OIDC token presented at the API | P13.02 server-side JWT validation (record `02_p13_02_oidc_token_validation.md`); P13.01 claims mapping resolved from the verified token, never request fields | WRITTEN-UNRUN; runtime IdP UNDEPLOYED — no token has ever been validated in operation |
| Tampering | Per-tenant SSO connection row altered to redirect authentication | P13.01: connections are tenant-scoped rows under RLS discipline; state changes are audited data changes | Depends on the RLS posture (P11.19 record) — same UNRUN status |
| Repudiation | Admin denies having changed identity configuration | P11.15 keyed audit chain (record `31_p11_15_audit_chain_upgrade.md`); P13.12 export | Chain is WRITTEN-UNRUN; un-keyed = chain columns NULL (honest state) |
| Information Disclosure | Identity claims (group membership) leaked across tenants | P0.16 tenancy principles; P11.19 RLS/pooling reconciliation; P11.08 log redaction | WRITTEN-UNRUN |
| Denial of Service | IdP outage blocks authentication | P13.01 architecture treats the IdP as external; session design (P13.16) carries revocation semantics | No offline-authn guarantee is claimed; no operating IdP exists to degrade |
| Elevation of Privilege | Role escalation via group-claim manipulation | P13.05 role model; P13.06 approval workflow; step-up P13.04 (P13-E-3 OPEN) | The never-granted set stays byte-identical; no bypass is designed here |

### 4.2 SR-2 — Protocol adapter surface (P14)

| STRIDE | Design threat on this surface | Existing control (cited) | Residual note |
|---|---|---|---|
| Spoofing | Device/endpoint impersonating a peer on a protocol channel | P14.02 fail-closed `MtlsRequirements`; P14.03 CA-chain/certificate-lifecycle design | P14.03 is DESIGN-ONLY — no CA stood up, no key material exists |
| Tampering | Malicious or incorrect telemetry injected through an adapter | P0.17 §5 threat 3 controls (source identity, validation, quality flags); P14.21 protocol observability | WRITTEN-UNRUN |
| Repudiation | Adapter operator denies a relaying decision | P11.15 audit chain over control-plane actions; P14.21 observability | WRITTEN-UNRUN |
| Information Disclosure | Cross-tenant telemetry leakage via an adapter | P14.16 binding model (one adapter, one identity); P0.17 §5 threat 4 controls (tenant context, RLS) | WRITTEN-UNRUN |
| Denial of Service | Protocol floods exhausting the aggregator | P14.20 backpressure/rate limiting at the adapter seam | WRITTEN-UNRUN |
| Elevation of Privilege | Adapter acquiring dispatch authority | P14.16 command-relay-only tripwire; `AuthorityDeclaration::SupervisoryRelayOnly` refuses dispatch variants; ADR-010-EXT-01; never-granted set byte-identical | Kernel-level tripwire, WRITTEN-UNRUN |

### 4.3 SR-3 — Market records surface (P16)

| STRIDE | Design threat on this surface | Existing control (cited) | Residual note |
|---|---|---|---|
| Spoofing | Forged market event/settlement statement ingested as a record | P16.01 closed `source_format` vocabulary; records as DECLARED data (ADR-011 posture), never asserted truth | WRITTEN-UNRUN; no market connected |
| Tampering | Settlement figures altered post-ingestion | P16.09: the engine never adjusts; P16.01 audit-chain linkage on every row | WRITTEN-UNRUN |
| Repudiation | Dispute over what was recorded/performed | P16.12 dispute workflow; P16.11 performance evidence (record-only) | WRITTEN-UNRUN |
| Information Disclosure | Market/business-sensitive data crossing jurisdictions | P16.18 jurisdiction register; P13.14 registry | WRITTEN-UNRUN; D1 OPEN — no jurisdiction constraint exists |
| Denial of Service | Price-signal ingestion stall | P16.07/P16.08 pipeline records; P14.20 backpressure upstream | WRITTEN-UNRUN |
| Elevation of Privilege | Bid submission authority appearing outside operator approval | P16.19: `market.bid.submit` deliberately ABSENT (pinned unparseable by test); grants require operator-approved mode + ACTIVE grant record | WRITTEN-UNRUN; no bid can be submitted — none can be |

### 4.4 SR-4 — OTA / edge fleet surface (P18)

| STRIDE | Design threat on this surface | Existing control (cited) | Residual note |
|---|---|---|---|
| Spoofing | Rogue device joining the fleet | P18.08 device identity lifecycle; P18.09 zero-touch onboarding design; P18.13 attestation | WRITTEN-UNRUN; no attestation ever performed |
| Tampering | Malicious/mis-signed update bundle | P18.02 signed manifest v2 with sequence-number anti-rollback; P18.15 update command interlock (updates ride the command lifecycle) | WRITTEN-UNRUN; no bundle ever produced or signed |
| Repudiation | "The update was never installed" disputes | P18.07 per-device digest-chained update evidence records | WRITTEN-UNRUN; no evidence record exists |
| Information Disclosure | Fleet topology/device metadata exposure | P18.12 fleet registry hardening | WRITTEN-UNRUN |
| Denial of Service | Failed rollout bricking a fleet segment | P18.01 ADR architecture; P18.06 rollout-orchestrator scope distinction; rollback evidence in P18.07 shapes | Design-level; no rollout has ever run |
| Elevation of Privilege | Update path used to push unauthorized behavior to devices | P18.15 interlock + the command lifecycle's three gates; invariant 6 (no unapproved target) and invariant 7 (ADR-010) apply unchanged | WRITTEN-UNRUN |

## 5. Zone mapping (design placement; P0.17 §3 frozen and unchanged)

| Surface | P0.17 zone placement (design) | Note |
|---|---|---|
| Identity (SR-1) | Zone 1 (enterprise IT) mediating into zone 2 (central application) | Federation terminates at the central application zone's validation middleware (P13.02) |
| Protocol adapters (SR-2) | Zone 3 (customer integration/DMZ) boundary toward zones 4–5 | The P0.17 §3 outbound-only flow rule and §3.3 topology reinforcement apply unchanged — no cloud-to-device access is designed |
| Market records (SR-3) | Zone 2 (central application) — records and pipelines only | No market surface reaches the edge; adapters relay commands only (SR-2 EoP row) |
| OTA (SR-4) | Spans zone 2 (orchestration) and zone 3 (edge agent) via the signed-bundle conduit | The bundle path is the conduit; the P0.17 §7 edge trust boundary applies: validation before any write, never a silent downgrade |

## 6. Invariants check — none of the ten §17.5 invariants is relaxed

The refresh adds surfaces; it adds no exception. In particular:
invariant 6 (no unapproved protocol adapter or target) now has the
P14.16 binding/manifest machinery as its designed enforcement; invariant
7 (no direct physical command from an optimizer, GNN, MARL, LLM, or
general plugin) is reinforced by ADR-010-EXT-01 (adapters hold no
dispatch authority — P14.16) and the market-side absence of any
dispatch action (P16.19); invariant 8 (no cross-tenant command or
telemetry access) is served by the same RLS/tenant-context controls
cited in §4. The ten invariants are reproduced at their frozen path,
never edited here.

## 7. Pen-test scope derivation (consumed by P19.13)

The four §3 surfaces ARE the pen-test scope classes: `records/24_p19_13_penetration_test_program.md`
§2 (SC-1 control plane ← SR-1; SC-2 edge agent ← SR-4; SC-3 console ←
SR-1; SC-4 protocol adapters ← SR-2; market records SR-3 are
in-scope-as-records via the control plane). The derivation is by
citation; P19.13 owns the engagement posture (NOT-STARTED cells).

## 8. Reviewer-verification procedure (human)

1. Confirm `ledger/phase-00-product-freeze/17_threat_model.md` is
   byte-identical (this batch touched only new files under this phase's
   `records/`).
2. Confirm every control citation in §4 resolves on disk and carries
   the producer's own status (WRITTEN-UNRUN / DESIGN RECORD / FROZEN).
3. Confirm §5 is a design placement and P0.17 §3 is unchanged.
4. Confirm §6 relaxes nothing and §7 derives P19.13's scope by citation.
5. Confirm the STRIDE methodology note (§2.1 ASSUMPTION) is present.

## 9. Status

**WRITTEN / DESIGN-ONLY.** The frozen P0.17 model stands; the four new
surfaces are registered with a STRIDE pass each, zone-mapped, and
invariant-checked; the pen-test scope derives from these surfaces by
citation. No threat is claimed mitigated in practice; no security
posture, certification, or audit outcome is claimed.
